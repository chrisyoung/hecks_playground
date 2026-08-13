require_relative "saga_interpreter/correlation"
require_relative "../bluebook/ir/process_manager"
require_relative "errors"
require_relative "value"

module Hecksagain
  module Runtime
    class SagaInterpreter
      include Correlation

      # The trigger lives on the declaration it triggers, not on the runtime that
      # notices it — see IR::ProcessManager::REFUSED.
      REFUSED = Bluebook::IR::ProcessManager::REFUSED

      attr_reader :registry

      def initialize(registry, door:)
        @registry = registry
        @door     = door
      end

      def advance(event, domain)
        bluebook = @registry.bluebook(domain)
        return unless bluebook

        bluebook.process_managers.each do |pm|
          begin_saga(pm, event)
          advance_saga(pm, event, domain)
          end_saga(pm, event)
        end
      end

      private

      def begin_saga(pm, event)
        return unless event.name == pm.starts_on

        correlation = saga_correlation(pm, event)
        if correlation.to_s.empty?
          @registry.saga_log << { process_manager: pm.name, on: event.name,
                                  born: false, reason: "no #{pm.correlates_by} in the payload" }
          return
        end
        return if @registry.saga_instances[pm.name].key?(correlation)

        # `.dup`, not the SAME Hash `event.payload` already is — a real
        # aliasing bug found live, not by inspection: `remember_into_
        # instance` (below) mutates `instance[:memory]` in place, and
        # without the dup that write landed on the STARTING event's own
        # `.payload` object too, since assignment here never copied it.
        # A `remember` declared on the SAME leg that starts the saga
        # (`on "TransferRequested"`, banking.bluebook's own Settlement)
        # retroactively added a field to an event already emitted and
        # logged — the one event a saga instance's memory must never be
        # able to reach backward and edit.
        @registry.saga_instances[pm.name][correlation] =
          { state: pm.states.first, memory: event.payload.dup }
        @registry.saga_log << { process_manager: pm.name, on: event.name,
                                instance: correlation, born: true, state: pm.states.first }
      end

      def advance_saga(pm, event, domain)
        handler = pm.handler_for(event.name)
        return unless handler

        correlation = saga_correlation(pm, event)
        return if correlation.to_s.empty?

        instance = @registry.saga_instances[pm.name][correlation]
        record   = { process_manager: pm.name, on: event.name, instance: correlation }

        unless instance
          @registry.saga_log << record.merge(advanced: false, reason: "no conversation remembers #{correlation.inspect}")
          return
        end
        unless instance[:state] == handler.from_state
          @registry.saga_log << record.merge(advanced: false,
                                             reason: "in #{instance[:state].inspect}, not #{handler.from_state.inspect}")
          return
        end

        instance[:state] = handler.to_state
        @registry.saga_log << record.merge(advanced: true, from: handler.from_state, to: handler.to_state)

        remember_into_instance(handler, event, instance, correlation)

        unless guards_pass?(handler, event, instance)
          @registry.saga_log << record.merge(dispatched: false, reason: "given clause did not hold")
          return
        end

        handler.dispatches.each do |spec|
          deliver_saga_dispatch(pm, spec, event, instance, correlation, domain)
        end
      end

      # Vendored addition, not (yet) upstream hecksagain (migration plan
      # task 4) — see HandlerBuilder#given's own comment. `ctx` merges the
      # triggering event's payload UNDER the instance's remembered
      # fields, so a `remember`ed value (this same firing's, per above,
      # since remember runs first) wins on key collision — the more
      # specific, more recently-decided fact.
      def guards_pass?(handler, event, instance)
        return true if (handler.guards || []).empty?

        ctx = event.payload.merge(instance[:memory])
        handler.guards.all? { |predicate| predicate.call(ctx) }
      end

      # Vendored addition, not (yet) upstream hecksagain (migration plan
      # task 4): resolves `handler.remembers` (from HandlerBuilder#remember)
      # against THIS firing's event/instance and merges them into
      # `instance[:memory]` -- BEFORE this handler's own dispatches run,
      # so a `remember key: from_event(:x)` and a same-handler `dispatch
      # ..., with: { y: from_pm(:key) }` compose in the natural written
      # order. No `row:` context -- remembers fire once per handler
      # firing, never once per for_each iteration.
      def remember_into_instance(handler, event, instance, correlation)
        return if (handler.remembers || []).empty?

        handler.remembers.each do |key, value|
          resolved = if !value.is_a?(Symbol) then value
                     elsif value == correlation then correlation
                     else resolve_with_value(value, event, instance)
                     end
          instance[:memory][key.to_sym] = resolved
        end
      end

      # Vendored addition, not (yet) upstream hecksagain (migration plan
      # task 4, i225): `spec.for_each` fans this dispatch out over a
      # named query's rows instead of firing once. `dispatch "Store.
      # PromoteSignal", for_each: { from: "Signal.cold" }, with: { kind:
      # from_iter(:kind), ... }` -- one Store.PromoteSignal per row
      # `Signal.cold` (a canned query) returns, each row's fields
      # resolved via `from_iter`. Zero rows is a legitimate no-op, not an
      # error -- an empty sweep is the common case (nothing cold this
      # tick), so it's silently skipped the same way a plain dispatch
      # with no correlation would be. `qualified` reused unchanged for
      # both the query FQN and the command FQN -- both are `Aggregate.
      # verb` within the SAME domain the process manager itself lives in.
      def deliver_saga_dispatch(pm, spec, event, instance, correlation, domain)
        unless spec.for_each
          deliver_one(pm, spec, event, instance, correlation, domain, iter: nil)
          return
        end

        rows = @door.query(qualified(spec.for_each, domain))
        rows.each { |row| deliver_one(pm, spec, event, instance, correlation, domain, iter: row) }
      end

      def deliver_one(pm, spec, event, instance, correlation, domain, iter:)
        args   = dispatch_args(pm, spec, event, instance, correlation, iter: iter)
        record = { process_manager: pm.name, instance: correlation, dispatch: spec.command_name }

        if @door.reaction_depth_reached?
          @registry.saga_log << record.merge(delivered: false,
                                             reason: "reaction depth #{@door.max_reaction_depth} reached")
          return
        end

        begin
          @door.reenter(qualified(spec.command_name, domain),
                        saga_correlation: { pm.correlation_head.to_s => correlation }, **args)
          @registry.saga_log << record.merge(delivered: true)
        rescue *DOMAIN_REFUSALS => error
          # Same rule as the policy interpreter : a refusal by the target is
          # a recorded outcome, and the leg that raised it UNWINDS — see
          # `unwind`'s own comment for why the procedure runs its
          # compensation here rather than leaving the money (or whatever
          # else a leg moved) sitting out.
          @registry.saga_log << record.merge(delivered: false, reason: error.message)
          unwind(pm, event, instance, correlation, domain)
        rescue StandardError => error
          # A DEFECT, not a refusal — see PolicyInterpreter#deliver's own
          # comment for the full reasoning: the same DOMAIN_REFUSALS split,
          # and the same "the triggering command already succeeded and
          # persisted by the time this runs" fact that makes catching it
          # here safe rather than reckless. Recorded distinguishably
          # (`defect: true`, plus the error's own class) and warned to
          # STDERR rather than left to blow up the ORIGINAL dispatch that
          # started this saga leg.
          #
          # DELIBERATELY DOES NOT `unwind`, unlike the branch above — that
          # is this method's one asymmetry with the policy interpreter's.
          # `unwind` runs the domain's OWN `on :refused` compensation, which
          # exists to undo a leg the domain itself declined. A crash is not
          # a decision the domain made ; running the compensating dispatch
          # for it would misrepresent what happened (there was no refusal
          # to compensate for) and could dispatch a real command against
          # state nobody actually decided to change. The instance is left
          # exactly where it landed, for a human to find via the warning
          # and the log.
          warn "[hecksagain] defect in saga #{pm.name} — instance #{correlation.inspect} " \
               "dispatching #{spec.command_name}: #{error.class}: #{error.message}"
          @registry.saga_log << record.merge(delivered: false, reason: error.message,
                                             defect: true, error_class: error.class.name)
        end
      end

      # A refused leg UNWINDS — the procedure runs the leg declared `on :refused`,
      # which is where the compensation lives.
      #
      # Until this existed a refusal was RECORDED and nothing else happened. The
      # wire's thousand was taken from the source, refused by the destination, and
      # sat nowhere until a human dispatched the reversal by hand ; banking's
      # settlement left a debit standing with no credit and no reversal at all.
      # Both bluebooks had written the compensating leg. Nothing armed it.
      #
      # A compensation that is itself refused does NOT unwind again, and needs no
      # flag to stop it: the state moves to the compensating leg's to_state BEFORE
      # its dispatches run, so a second refusal finds the instance no longer in
      # from_state and records that instead. The check is the guard.
      def unwind(pm, event, instance, correlation, domain)
        handler = pm.handler_for(REFUSED)
        return unless handler && instance

        record = { process_manager: pm.name, on: REFUSED, instance: correlation }
        unless instance[:state] == handler.from_state
          @registry.saga_log << record.merge(advanced: false,
                                             reason: "in #{instance[:state].inspect}, not #{handler.from_state.inspect}")
          return
        end

        instance[:state] = handler.to_state
        @registry.saga_log << record.merge(advanced: true, from: handler.from_state, to: handler.to_state)

        handler.dispatches.each do |spec|
          deliver_saga_dispatch(pm, spec, event, instance, correlation, domain)
        end
      end

      def dispatch_args(pm, spec, event, instance, correlation, iter: nil)
        spec.with_spec.to_h do |key, value|
          resolved = resolve_value(value, pm, event, instance, correlation, iter)
          # A process manager carries facts between aggregate boundaries.  It
          # must carry a value object's state, not its source aggregate's
          # runtime type: TransferMoney and Account::Money may share fields
          # without being the same domain object.
          [key.to_sym, Value.materialize(resolved)]
        end
      end

      # Vendored addition, not (yet) upstream hecksagain (migration plan
      # task 4): pulled out of `dispatch_args` unchanged, plus one new
      # branch -- `Bluebook::IR::TemplateSpec` resolves each of its OWN
      # args through this SAME method (recursively; a template arg can
      # itself be `pm.correlation_head` or a plain field reference,
      # anything an ordinary `with:` value could be) and substitutes into
      # `format`. Fully qualified (not bare `IR`, unlike
      # process_manager_builder.rb's own `template` sugar): THIS class
      # lives under `Hecksagain::Runtime`, a SIBLING of `Hecksagain::
      # Bluebook`, not nested inside it, so bare `IR` never resolves here
      # the way it does from inside `Hecksagain::Bluebook::DSL` — this
      # branch was never dispatched for real until burning-man-prep's own
      # `PlaceNewItemOnOwnersPersonalList` saga hit it (migration
      # investigation, 2026-08-11), raising `uninitialized constant
      # Hecksagain::Runtime::SagaInterpreter::IR` on every `with:` value
      # that happened to need a plain (non-template) resolution too,
      # since the `case` itself never got past the `when` clause's own
      # constant lookup.
      def resolve_value(value, pm, event, instance, correlation, iter)
        case value
        when Bluebook::IR::TemplateSpec
          resolved_args = value.args.map { |arg| resolve_value(arg, pm, event, instance, correlation, iter) }
          format(value.format, *resolved_args)
        when Symbol
          value == pm.correlation_head ? correlation : resolve_with_value(value, event, instance, row: iter)
        else
          value
        end
      end

      # A DOTTED SOURCE READS THE ONE SCALAR, the same "a path names a
      # field, not a whole value object" idiom `identified_by`/
      # `correlates_by` already hold every other declaration to —
      # `with: { item_id: :"id.value" }` reaches into a VO-wrapped
      # payload field (Item.Add's own `id: ItemId`) for the bare scalar
      # a reference-typed target argument (Place's own `item_id`)
      # actually needs, rather than handing it the whole `{value: "..."}`
      # shape a plain top-level key would. A bare (undotted) source is
      # unchanged from before this existed — reads the field whole,
      # exactly as every existing `with:` mapping in the corpus already
      # does.
      # `row:`, vendored addition not (yet) upstream hecksagain (migration
      # plan task 4, i225): a for_each dispatch's iteration record, tried
      # FIRST -- `from_iter`/`from_event` are indistinguishable at the IR
      # level (both HandlerBuilder methods just return the bare field
      # Symbol, see process_manager_builder.rb's own comment), so this is
      # a runtime-side, not parse-side, resolution: when a row is present
      # (a for_each dispatch) and carries the field, it wins over the
      # event/instance fallback every other dispatch already used.
      def resolve_with_value(value, event, instance, row: nil)
        head, *rest = value.to_s.split(".").map(&:to_sym)
        base = if row && row.key?(head)
                 row[head]
               elsif event.payload.key?(head)
                 event.payload[head]
               else
                 instance[:memory][head]
               end
        return base if rest.empty?

        rest.reduce(Value.materialize(base)) { |held, segment| held.is_a?(Hash) ? held[segment] : held }
      end

      def qualified(command_name, domain)
        command_name.include?("::") ? command_name : "#{domain}::#{command_name}"
      end

      def end_saga(pm, event)
        return unless event.name == pm.ends_on

        correlation = saga_correlation(pm, event)
        return if correlation.to_s.empty?
        return unless @registry.saga_instances[pm.name].delete(correlation)

        @registry.saga_log << { process_manager: pm.name, on: event.name,
                                instance: correlation, ended: true }
      end
    end
  end
end
