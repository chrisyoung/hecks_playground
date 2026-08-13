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

module Hecksagain
  module Runtime
    class Dispatcher
      MAX_REACTION_DEPTH = 5

      # THE DOMAIN'S OWN OUTCOME CHAIN — where an execution-port reply is
      # recorded once the adapter has run. Cascade's own bluebook already
      # declares exactly this ("After a ShellTool / FileTool / SearchTool
      # dispatch runs through its adapter, the adapter chains
      # Cascade.RecordResult carrying the captured outcome"), so the
      # dispatcher re-enters that command rather than inventing a second
      # place for tool outcomes to live.
      CASCADE_VERB = "Cascade::Cascade.RecordResult".freeze

      # `reply` is the execution port's returned value (Ports::Execution) —
      # nil for the overwhelming majority of dispatches, which touch no
      # impure edge at all. It rides on the Result because `events` alone
      # cannot carry it: a re-entered command's events go into the registry's
      # event log, not into this call's `announced`, so a caller reading only
      # events would watch a shell command run and still see nothing.
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
              @entities.call(domain, aggregate, command_name, args)
            end
          else
            command = aggregate.command(command_name) ||
                      raise(UnknownVerb, RefusalWording.render("UnknownVerb", "aggregate_no_command",
                                                                aggregate: aggregate_name, command: command_name.inspect))
            @commands.call(domain, aggregate, command, args)
          end

        # STAMPED BEFORE reactions and sagas see these events, not after —
        # `SagaInterpreter#advance` runs on THIS domain's `announced` events
        # within this very call, and a step further down the same saga has to
        # find the stamp already there. See `Event#correlation`'s own comment.
        if saga_correlation
          announced.each { |event| (event.correlation ||= {}).merge!(saga_correlation) }
        end

        # THE IMPURE EDGE, AFTER THE DOMAIN HAS AGREED — the command has
        # already passed its givens, its role gate and its ensures, and the
        # record is saved. Only then does the bound adapter actually run, so
        # a refused command never reaches the world. A port operation
        # (`command_name` carrying a dot) is skipped: it is an adapter
        # calling IN, and running an adapter for it would invert the
        # direction the port exists to express.
        reply = command_name.include?(".") ? nil : perform_execution(domain, aggregate, command_name, args)
        announced.concat(record_outcome(reply, args)) if reply

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

      # Vendored addition, not (yet) upstream hecksagain — see
      # ports/execution.rb for the whole story. An aggregate with no
      # `executed_by` bind returns nil here and nothing changes for it,
      # which is every aggregate in every domain but the tool ones.
      def perform_execution(domain, aggregate, command_name, args)
        Ports::Execution.perform(@registry, domain, aggregate, command_name, args)
      end

      # An adapter that ran is a fact about the domain, so it is recorded as
      # one. Guarded on Cascade actually being loaded — a corpus that binds
      # an execution adapter without Cascade in it still gets its reply back
      # on the Result, it just has no outcome chain to write to.
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

      # i746 — the effect-port producer. Port of rust/src/runtime/
      # effect_outbound.rs's record_effect_outbound (IR shape:
      # rust/src/hecksagon_ir.rs:187-209). For each just-announced event,
      # for every hexagon's every bind whose `on` matches this event and
      # whose adapter resolves to an EFFECT-signal port (charged_by, not
      # persisted_by), records ONE OutboundEvent::OutboundEvent.Record
      # carrying the qualified success/failure verdict commands — a
      # standalone host (bin/adapter-host) later claims it, does the
      # impure async work, and dispatches the verdict back in. The core
      # never waits.
      #
      # Guarded on OutboundEvent actually being attached (uses_framework
      # "OutboundEvent") — same precedent as record_outcome's own Cascade
      # guard just above — so a domain that never wired the framework in
      # pays nothing and a bind with no `on` (the overwhelming majority)
      # short-circuits on the very next line.
      #
      # port_for is GUARDED, not called bare : it raises WiringError on an
      # unresolvable adapter/port, but Rust's own equivalent `continue`s
      # past a broken bind (effect_outbound.rs) rather than aborting the
      # whole dispatch over one bad binding elsewhere in the corpus.
      def record_effect_outbound(announced)
        return if announced.empty?
        return unless @registry.bluebook("OutboundEvent")

        seen_delivery_ids = []

        announced.each do |event|
          @registry.hecksagons.each_value do |hexagon|
            hexagon.binds.each do |bind|
              next if bind.on.to_s.empty? || bind.on.to_s != event.name.to_s
              next if bind.success.to_s.empty? && bind.failure.to_s.empty?

              port = begin
                @registry.port_for(bind)
              rescue WiringError
                next
              end
              next unless port.verb.to_s == bind.verb.to_s
              next unless port.effect?

              delivery_id = "#{event.aggregate}::#{event.id}::#{event.name}::#{bind.adapter}"
              next if seen_delivery_ids.include?(delivery_id)

              seen_delivery_ids << delivery_id

              bind_domain = bind.aggregate.to_s.split("::").first
              qualify = lambda do |cmd|
                cmd = cmd.to_s
                cmd.empty? || cmd.include?("::") ? cmd : "#{bind_domain}::#{cmd}"
              end

              reenter(
                "OutboundEvent::OutboundEvent.Record",
                delivery_id:     delivery_id,
                adapter:         bind.adapter.to_s,
                event:           event.name.to_s,
                source_type:     event.aggregate.to_s,
                source_id:       event.id.to_s,
                payload:         event.payload.to_json,
                success_command: qualify.call(bind.success),
                failure_command: qualify.call(bind.failure)
              )
            end
          end
        end
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
