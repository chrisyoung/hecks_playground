require_relative "errors"
require_relative "refusal_wording"
require_relative "caller"
require_relative "command_rules"
require_relative "command_interpreter"
require_relative "entity_interpreter"
require_relative "query_interpreter"
require_relative "read_model_interpreter"
require_relative "policy_interpreter"
require_relative "saga_interpreter"
require_relative "../naming"
require "json"

module Hecks
  module Runtime
    class Dispatcher
      MAX_REACTION_DEPTH = 5

      # THE DOMAIN'S OWN OUTCOME CHAIN — where an execution-port reply is
      # recorded once the adapter has run. A consuming corpus's own Cascade
      # bluebook (documented in that corpus, not this gem — see
      # Ports::Execution) is the intended landing site: after a
      # ShellTool/FileTool/SearchTool/EmailTool dispatch runs through its
      # bound adapter, the dispatcher chains this command carrying the
      # captured outcome. Guarded on Cascade actually being loaded
      # (`record_outcome`, below) — a corpus with no Cascade bluebook still
      # gets its reply back on the Result, it just has no outcome chain to
      # write to.
      CASCADE_VERB = "Cascade::Cascade.RecordResult".freeze

      # THE EFFECT PORT'S PRODUCER — record_effect_outbound (below). A
      # consuming corpus's own OutboundEvent bluebook (documented in that
      # corpus, not this gem — see Bind's own on:/success:/failure: comment
      # in bluebook/hexagon.rb) is the durable outbox this dispatches into :
      # one Record per Bind subscribing to the event just emitted, delivered
      # out-of-process by a standalone host (bin/adapter-host), never the
      # core itself. Guarded on OutboundEvent actually being loaded, same
      # shape as CASCADE_VERB/record_outcome above — a corpus with no
      # OutboundEvent bluebook simply has no effect-port binds that could
      # ever populate one.
      OUTBOUND_EVENT_VERB = "OutboundEvent::OutboundEvent.Record".freeze

      Result = Struct.new(:verb, :instance, :events, :reply, keyword_init: true) do
        # `instance` is nil for a port operation dispatched by verb (below)
        # — nothing was hydrated or saved, the same reason
        # `PortOperationInterpreter#emit`'s own comment gives for sourcing
        # `id:` off the operation's reference attribute instead. `&.`, not
        # a raised error: a caller that dispatches a port verb and then
        # asks this Result for `.id`/`.state` made a category error the
        # domain itself already told it about (there is no record here),
        # not a crash-worthy one.
        def id    = instance&.id
        def state = instance&.to_h

        def to_s
          announced = events.empty? ? "no events" : events.map(&:name).join(", ")
          "#{verb} → #{instance.inspect} | #{announced}"
        end

        def inspect = "#<Result #{self}>"
      end

      attr_reader :registry

      def initialize(registry)
        @registry = registry
        rules     = CommandRules.new(registry)
        @commands  = CommandInterpreter.new(registry, rules: rules)
        @port_ops  = PortOperationInterpreter.new(registry, rules: rules)
        @entities = EntityInterpreter.new(registry, rules: rules)
        @queries  = QueryInterpreter.new(registry)
        @read_models = ReadModelInterpreter.new(registry)
        @policies = PolicyInterpreter.new(registry, door: self)
        @sagas    = SagaInterpreter.new(registry, door: self)
      end

      def events = @registry.event_log

      def reactions = @registry.reaction_log

      def sagas = @registry.saga_log
      def saga_dispatches = @registry.saga_dispatch_log
      def policy_dispatches = @registry.policy_dispatch_log
      def verbs = @registry.verbs

      def dispatch(verb, saga_correlation: nil, **args)
        domain, aggregate_name, command_name = parse(verb)
        aggregate = resolve_aggregate(domain, aggregate_name, verb)

        instance, announced =
          if command_name.include?(".")
            head, sub = command_name.split(".", 2)
            port = aggregate.port(head)
            # A PORT OPERATION, reached by the SAME verb shape an entity
            # command already uses ("Domain::Aggregate.Head.Rest") — ports
            # are checked first, so an aggregate that ever declared both a
            # port and an entity of the same name would resolve to the
            # port; no domain in this corpus does, and `dispatch_port`'s
            # own header already named this as an open wire-spelling
            # question this resolves, not silently avoids. No `instance`
            # comes back — nothing is hydrated or saved by a port
            # operation (`PortOperationInterpreter`'s own header) — so
            # `Result#id`/`#state` are nil-safe (above) for exactly this
            # path.
            if port
              operation = port.operation(sub) ||
                          raise(UnknownVerb, RefusalWording.render("UnknownVerb", "port_no_operation",
                                                                    port: head, operation: sub.inspect))
              [nil, @port_ops.call(domain, aggregate, operation, args)]
            else
              # THE ENTITY BOUNDARY — an entity's own command shares this
              # dotted VERB SHAPE, but never resolves through it: dispatched
              # BY VERB STRING, an entity command refuses UNCONDITIONALLY,
              # not merely outside a reaction. There is no carve-out for
              # `reenter` either — that method calls straight back into
              # THIS one (below), so a policy/saga's own trigger can no
              # longer name an entity command any more than an external
              # caller can. `Dispatcher#dispatch_entity` is the ONLY door
              # that reaches one — never a verb a caller wrote out as
              # text, always a direct in-process call (the facade's own
              # generated `Aggregate::Entity.command!` methods are the
              # ordinary way one gets made ; see docs/guides/entities.md).
              # `role:` (LedgerEntry.Amend's own `role "Back office"`, in
              # the banking example) still checks WHO may run this once
              # `dispatch_entity` is reached — see EntityDispatchRefused's
              # own header for why that is a narrower question now than
              # whether an outside caller may reach this AT ALL.
              raise EntityDispatchRefused, RefusalWording.render("EntityDispatchRefused", "direct_dispatch",
                                                                  entity: head, aggregate: aggregate_name)
            end
          else
            command = aggregate.command(command_name) ||
                      raise(UnknownVerb, RefusalWording.render("UnknownVerb", "aggregate_no_command",
                                                                aggregate: aggregate_name, command: command_name.inspect))
            @commands.call(domain, aggregate, command, args, saga_correlation)
          end

        # Correlation is SET AT CONSTRUCTION now, not merged on here —
        # it is part of the transaction, known from this method's own
        # argument before a single event exists. It used to be stamped
        # onto already-emitted events, which is what kept an event
        # mutable after it had happened.
        #
        # The ordering this note used to guard still holds, and more
        # simply: `SagaInterpreter#advance` runs on THIS domain's
        # `announced` events within this very call, and finds the
        # correlation already there because it was never absent.

        # THE IMPURE EDGE, AFTER THE DOMAIN HAS AGREED — the command has
        # already passed its rules and the record is saved (or, for an
        # entity command, its container has). Only then does the bound
        # execution adapter actually run, so a refused command never
        # reaches the world. Skipped for ANY dotted `command_name` — a
        # port operation dispatched by verb (an adapter calling IN, where
        # running an adapter for it would invert the direction the port
        # exists to express) or an entity command (no aggregate-level
        # `executed_by` bind resolves for those; `Ports::Execution.perform`
        # itself would just find nothing bound and return nil, but the
        # dotted check keeps that intent explicit rather than incidental).
        reply = command_name.include?(".") ? nil : perform_execution(domain, aggregate, command_name, args)
        announced.concat(record_outcome(reply, args)) if reply

        record_driven_dispatches(announced)
        record_effect_outbound(announced)

        announced.each { |event| @policies.react(event, domain) }

        announced.each { |event| @sagas.advance(event, domain) }

        Result.new(verb: verb, instance: instance, events: announced, reply: reply)
      end

      # THE DOOR AN ADAPTER OUTSIDE THE BLUEBOOK CALLS THROUGH — never the
      # domain itself. `port_name`/`operation_name` are separate arguments
      # rather than one packed verb string on purpose: there is no established
      # wire spelling for "domain, aggregate, port, operation" yet, and
      # inventing one is a bigger decision than this call needs to make.
      #
      # No adapter-to-port binding lookup happens here — that is
      # `Hecks.adapter`'s existing job (unchanged by this), and wiring "which
      # adapter may call this port" through is the next piece, not this one.
      def dispatch_port(domain, aggregate_name, port_name, operation_name, **args)
        aggregate = resolve_aggregate(domain, aggregate_name, "#{domain}::#{aggregate_name}.#{port_name}.#{operation_name}")
        port = aggregate.port(port_name) ||
               raise(UnknownVerb, "#{aggregate_name} has no port #{port_name.inspect}")
        operation = port.operation(operation_name) ||
                    raise(UnknownVerb, "#{port_name} has no operation #{operation_name.inspect}")

        announced = @port_ops.call(domain, aggregate, operation, args)

        announced.each { |event| @policies.react(event, domain) }
        announced.each { |event| @sagas.advance(event, domain) }

        announced
      end

      # THE ONLY DOOR THAT REACHES A NESTED ENTITY'S OWN COMMAND —
      # `dispatch`, above, refuses `Domain::Aggregate.Entity.Command`
      # UNCONDITIONALLY, verb-string dispatch never resolves one at all.
      # This is the door instead: a DIRECT in-process call, never a verb
      # a caller wrote out as text and handed to `dispatch`/`reenter` —
      # the facade's own generated `Aggregate::Entity.command!` methods
      # (facade/surface/entity_door.rb) are the ordinary way one gets
      # called, application code and a domain's OWN other command bodies
      # calling it directly being the rest.
      #
      # Same verb STRING GRAMMAR `dispatch` itself parses (`parse`,
      # `Naming.split_verb`) — not a second grammar, the same one,
      # entered through a different door, the identical reason
      # `dispatch_port` reuses `resolve_aggregate` rather than inventing
      # its own aggregate lookup.
      def dispatch_entity(verb, **args)
        domain, aggregate_name, dotted = parse(verb)
        aggregate = resolve_aggregate(domain, aggregate_name, verb)

        instance, announced = @entities.call(domain, aggregate, dotted, args)

        announced.each { |event| @policies.react(event, domain) }
        announced.each { |event| @sagas.advance(event, domain) }

        Result.new(verb: verb, instance: instance, events: announced)
      end

      # A READ-ONLY CLASSIFIER, not a second copy of `dispatch`'s own
      # port-vs-entity branch — a dotted verb is ambiguous on its own
      # (a port operation and an entity command share the identical
      # `Domain::Aggregate.Head.Rest` shape), and a caller holding a
      # step list of verb strings (`Hecks::Fuzzing::Replay`'s own
      # corpus replay, the one caller today) needs to know which DOOR
      # to call — `dispatch` for a port, `dispatch_entity` for an
      # entity — before it can call either one. `false` for a plain
      # (non-dotted) verb or an unresolvable one: nothing this predicate
      # would refuse FOR, that's `dispatch`'s/`dispatch_entity`'s own job.
      def entity_command?(verb)
        domain, aggregate_name, command_name = parse(verb)
        return false unless command_name.include?(".")

        aggregate = resolve_aggregate(domain, aggregate_name, verb)
        head = command_name.split(".", 2).first
        aggregate.port(head).nil?
      rescue UnknownVerb
        false
      end

      def query(verb, **args)
        domain, query_name = verb.to_s.split(".", 2)
        if query_name && !domain.include?("::")
          bluebook = @registry.bluebook(domain) ||
                     raise(UnknownVerb, RefusalWording.render("UnknownVerb", "no_domain", domain: domain.inspect, verb: verb))
          model = bluebook.read_model(query_name) ||
                  raise(UnknownVerb, RefusalWording.render("UnknownVerb", "no_read_model",
                                                            domain: domain, query: query_name.inspect))
          return @read_models.call(domain, model, args)
        end

        domain, aggregate_name, query_name = parse(verb)
        aggregate = resolve_aggregate(domain, aggregate_name, verb)

        @queries.call(domain, aggregate, query_name, args)
      end

      # The same ask, answered by the reference interpreter alone — never
      # the bound adapter's native hook. Read models have no reference
      # twin, so only the aggregate-query form answers here; the fuzzer's
      # query oracle diffs this against #query's answer.
      def reference_query(verb, **args)
        domain, aggregate_name, query_name = parse(verb)
        aggregate = resolve_aggregate(domain, aggregate_name, verb)

        @queries.reference_call(domain, aggregate, query_name, args)
      end

      # A reaction is the SYSTEM acting, not the caller who happened to be
      # on the stack when the triggering command ran — the ambient caller
      # is cleared for the reaction's own dispatch, so a triggering
      # caller's role can neither satisfy nor block a reaction command it
      # has nothing to do with (Runtime::Caller.without).
      def reenter(verb, saga_correlation: nil, **args)
        depth = @reaction_depth.to_i
        @reaction_depth = depth + 1
        Caller.without { dispatch(verb, saga_correlation: saga_correlation, **args) }
      ensure
        @reaction_depth = depth
      end

      def reaction_depth_reached? = @reaction_depth.to_i >= MAX_REACTION_DEPTH
      def max_reaction_depth      = MAX_REACTION_DEPTH

      private

      # See ports/execution.rb for the whole story. An aggregate with no
      # `executed_by` bind returns nil here and nothing changes for it,
      # which is every aggregate in every domain but the tool ones.
      def perform_execution(domain, aggregate, command_name, args)
        Ports::Execution.perform(@registry, domain, aggregate, command_name, args)
      end

      # An adapter that ran is a fact about the domain, so it is recorded
      # as one. Guarded on Cascade actually being loaded — a corpus that
      # binds an execution adapter without Cascade in it still gets its
      # reply back on the Result (above), it just has no outcome chain to
      # write to.
      #
      # Failures here are swallowed DELIBERATELY and reported in-band: the
      # adapter has already run and its output is already in hand, so
      # raising would throw away a real result over a bookkeeping problem.
      def record_outcome(reply, args)
        return [] unless @registry.bluebook("Cascade")

        reenter(CASCADE_VERB,
                id:        args[:id].to_s,
                tool:      reply[:tool].to_s,
                output:    reply[:output].to_s,
                exit_code: reply[:exit_code].to_i,
                ok:        reply[:ok] ? true : false).events
      rescue StandardError => error
        reply[:cascade_error] = "#{error.class}: #{error.message}"
        []
      end

      # The driven-side counterpart to record_outcome/perform_execution : a
      # driven handler is a direct in-process cross-context call, not a
      # port-mediated effect (`adapter "X" do driven on "Domain::Aggregate
      # .Event" do dispatch "..." ; success "..." ; failure "..." end
      # end`) — no port, no family, no adapter contract, so no
      # `Registry#port_for` check, just a plain string match against the
      # triggering event's own domain-qualified name. `success`/`failure`
      # are optional — a driven block declaring neither is fire-and-forget.
      def record_driven_dispatches(announced)
        return if announced.empty?

        announced.each do |event|
          qualified_event_name = "#{event.aggregate}.#{event.name}"

          @registry.hecksagons.each_value do |hexagon|
            hexagon.driven_handlers.each do |handler|
              next unless handler.event == qualified_event_name

              deliver_driven(handler, event)
            end
          end
        end
      end

      def deliver_driven(handler, event)
        handler_domain = event.aggregate.to_s.split("::").first

        if handler.for_each
          fan_out_driven(handler, event)
        else
          reenter(handler.dispatch_command, **interpolate_args(handler.dispatch_args || {}, event))
        end
        return if handler.success.to_s.empty?

        reenter(qualify_command(handler.success, handler_domain), id: event.id)
      rescue StandardError
        return if handler.failure.to_s.empty?

        reenter(qualify_command(handler.failure, handler_domain), id: event.id)
      end

      # THE DRIVEN SIDE'S FAN-OUT — the same treatment `PolicyInterpreter
      # #deliver_for_each` already gives a bluebook Policy and
      # `SagaInterpreter#deliver_saga_dispatch` gives a process manager,
      # given to a hecksagon's own `driven` block: `dispatch "Domain::
      # Aggregate.Command", for_each: { from: "Domain::Aggregate.query",
      # where: { field: "{event_field}" } }, with: { id: "{row_field}" }`
      # fires ONE dispatch PER ROW the named query answers, instead of one
      # per event. Before this the `for_each:` hash was captured and then
      # dropped, so every such handler fired exactly once and swept
      # nothing.
      #
      # `where:` is the QUERY'S arguments, `{field}`-interpolated against
      # the triggering event alone — the query is what decides the rows,
      # so no row exists yet to read from. `with:` is the DISPATCH'S
      # arguments, interpolated against the event AND the row, the row
      # winning on a name they share (the row is what this iteration is
      # about). That is the driven equivalent of the row key
      # `deliver_for_each` merges in under `addressing_key_for` — but
      # NAMED BY THE AUTHOR rather than derived, because a driven handler
      # already spells out every argument it sends and there is no
      # `trigger` for the runtime to interrogate.
      #
      # A ROW'S OWN REFUSAL DOES NOT STOP THE SWEEP, the same rule
      # `deliver_for_each` holds (it records `delivered: false` for that
      # row and carries on): a fan-out over N rows routinely finds some
      # already in the state it was going to move them to — a stale
      # worker marked dead twice, a lease already reclaimed — and one such
      # refusal must not strand the remaining rows. Anything that is NOT a
      # domain refusal — a query that does not resolve, a defect in an
      # interpreter — propagates to `deliver_driven`'s own rescue above
      # and lands on the handler's `failure` verdict, exactly as a
      # non-fan-out driven dispatch's would.
      def fan_out_driven(handler, event)
        spec = handler.for_each.transform_keys(&:to_sym)
        from = spec[:from] ||
               raise(UnknownVerb, "a driven dispatch's for_each names no from: — #{handler.for_each.inspect}")
        query_args = interpolate_args(spec[:where] || {}, event)

        Array(query(from, **query_args)).each do |row|
          reenter(handler.dispatch_command, **interpolate_args(handler.dispatch_args || {}, event, row: row))
        rescue *DOMAIN_REFUSALS
          next
        end
      end

      # A success/failure verdict command is written bare in the corpus
      # sometimes ("Order.Authorize") and already fully qualified other
      # times — only prefix when it's bare.
      def qualify_command(cmd, domain)
        cmd = cmd.to_s
        cmd.empty? || cmd.include?("::") ? cmd : "#{domain}::#{cmd}"
      end

      # THE EFFECT PORT'S PRODUCER — the counterpart to record_driven_
      # dispatches, but for a PORT-mediated effect bind (`Order.charged_by
      # ("Stripe", on: "OrderPlaced") do success "..." ; failure "..." end`,
      # and its :exec-family sibling `spawned_by`) rather than a driven
      # handler's direct in-process call. Matches on the SAME bare event
      # name shape Bind#on documents ("OrderPlaced", not domain-qualified),
      # scoped to the emitting aggregate — never a global name search, so
      # two different aggregates emitting an identically-named event never
      # cross-fire (this is what makes gap #1b's old collision concern moot
      # here : a Bind is aggregate-scoped by construction, a bluebook
      # policy's bare-name match is not).
      #
      # ONE OutboundEvent::OutboundEvent.Record PER MATCHING BIND, exactly
      # as that command's own goal text documents ("one Record per
      # subscribing adapter") — so N adapters bound to the same event (
      # process_health's reap + sweep both on ProcessMacrophage.Swept,
      # plan's four checks all on Story.StoryChecksRequested) each get
      # their own durable row, independently claimed and delivered.
      #
      # Guarded on OutboundEvent actually being loaded (see
      # OUTBOUND_EVENT_VERB's own comment). Recording failures are
      # swallowed per-bind, same reasoning as record_outcome : the emitting
      # command has already succeeded and saved, so a bookkeeping failure
      # for one subscriber must never take down the others or unwind an
      # already-committed dispatch.
      def record_effect_outbound(announced)
        return if announced.empty?
        return unless @registry.bluebook("OutboundEvent")

        announced.each do |event|
          @registry.hecksagons.each_value do |hexagon|
            hexagon.binds.each do |bind|
              next if bind.on.to_s.empty?
              next unless bind.aggregate.to_s == event.aggregate.to_s
              next unless bind.on.to_s == event.name.to_s

              deliver_effect_outbound(bind, event)
            end
          end
        end
      end

      def deliver_effect_outbound(bind, event)
        domain = bind.aggregate.to_s.split("::").first
        payload = (event.payload || {}).transform_values { |v| materialize_value(v) }

        reenter(OUTBOUND_EVENT_VERB,
                delivery_id:     "#{event.id}:#{bind.adapter}",
                adapter:         bind.adapter.to_s,
                event:           event.name.to_s,
                source_type:     bind.aggregate.to_s,
                source_id:       event.id.to_s,
                payload:         payload.to_json,
                success_command: qualify_command(bind.success, domain),
                failure_command: qualify_command(bind.failure, domain))
      rescue StandardError => error
        warn "[record_effect_outbound] failed to record #{bind.adapter} <- #{event.aggregate}.#{event.name}: " \
             "#{error.class}: #{error.message}"
      end

      # `{field}` interpolation for a driven handler's dispatch_args :
      # `{id}` resolves to the triggering event's own aggregate id ; any
      # other `{name}` resolves against the event's payload. A token that
      # IS the whole string ("{total}") substitutes the materialized value
      # verbatim (preserving its type — a multi-field VO's own Hash, not a
      # stringification of it, so it round-trips into another VO-typed
      # attribute on the receiving command) ; a token embedded in a larger
      # string interpolates as text.
      #
      # `row:` is a FAN-OUT ITERATION'S OWN ROW (`fan_out_driven`, above),
      # laid over the event's fields rather than beside them: a row field
      # and an event field sharing a name resolve to the ROW'S, because a
      # fan-out dispatch is about the row it is acting on. Absent — every
      # ordinary driven dispatch — the lookup is exactly what it was.
      def interpolate_args(args, event, row: nil)
        lookup = { "id" => event.id }
        (event.payload || {}).each { |k, v| lookup[k.to_s] = materialize_value(v) }
        (row || {}).each { |k, v| lookup[k.to_s] = materialize_value(v) }

        args.transform_values do |v|
          next v unless v.is_a?(String)

          if v =~ /\A\{(\w+)\}\z/
            lookup.key?(::Regexp.last_match(1)) ? lookup[::Regexp.last_match(1)] : v
          else
            v.gsub(/\{(\w+)\}/) { lookup.key?(::Regexp.last_match(1)) ? lookup[::Regexp.last_match(1)].to_s : ::Regexp.last_match(0) }
          end
        end.transform_keys(&:to_sym)
      end

      # A Runtime::Value wrapping a single :value field materializes to
      # that bare scalar (the common case — most dispatch_args tokens
      # reference a simple VO) ; a multi-field VO materializes to its
      # whole #to_h, structurally passed through to whatever VO-typed
      # attribute receives it on the other side.
      def materialize_value(value)
        return value unless value.respond_to?(:to_h)

        h = value.to_h
        h.size == 1 && h.key?(:value) ? h[:value] : h
      end

      def parse(verb)
        Naming.split_verb(verb) ||
          raise(UnknownVerb, RefusalWording.render("UnknownVerb", "not_fully_qualified", verb: verb.inspect))
      end

      def resolve_aggregate(domain, aggregate_name, verb)
        bluebook = @registry.bluebook(domain) ||
                   raise(UnknownVerb, RefusalWording.render("UnknownVerb", "no_domain", domain: domain.inspect, verb: verb))
        bluebook.aggregate(aggregate_name) ||
          raise(UnknownVerb, RefusalWording.render("UnknownVerb", "no_aggregate",
                                                    domain: domain, aggregate: aggregate_name.inspect))
      end
    end
  end
end
