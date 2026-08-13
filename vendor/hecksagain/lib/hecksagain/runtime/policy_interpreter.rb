require_relative "../naming"
require_relative "errors"
require_relative "query_interpreter"
require_relative "refusal_wording"


module Hecksagain
  module Runtime
    class PolicyInterpreter
      attr_reader :registry

      def initialize(registry, door:)
        @registry = registry
        @door     = door
      end

      def react(event, domain)
        policies_for(event, domain).each do |policy|
          # `deliver` returns one record ordinarily, or an ARRAY of
          # records for a `for_each` policy (vendored addition -- see
          # PolicyBuilder#for_each's own comment) -- one dispatch per
          # matched row, so one reaction-log entry per row too. NOT
          # `Array(result)` -- that turns a plain record Hash into its
          # own `[[key, value], ...]` pairs instead of wrapping it.
          result = deliver(policy, event, domain)
          (result.is_a?(Array) ? result : [result]).each { |record| @registry.reaction_log << record }
        end
      end

      private

      def policies_for(event, domain)
        bluebook = @registry.bluebook(domain)
        return [] unless bluebook

        emitting = Naming.demodulise(event.aggregate)

        bluebook.policies.select do |policy|
          policy.event_name == event.name &&
            (policy.event_qualifier.nil? || policy.event_qualifier == emitting) &&
            wheres_match?(policy, event)
        end
      end

      # `where field: value` -- vendored addition, not (yet) upstream
      # hecksagain (migration plan task 8): see PolicyBuilder#where's own
      # comment. A wheres-mismatch is NOT a refusal -- it means this
      # policy simply doesn't apply to this particular event, the same
      # as `policy.event_qualifier` not matching just above. Compared
      # against the RAW event payload (symbolized), not a stored record
      # -- a policy's where clause reads what the triggering event
      # itself carried, not the aggregate's current state.
      def wheres_match?(policy, event)
        return true if policy.wheres.nil? || policy.wheres.empty?

        payload = event.payload.transform_keys(&:to_sym)
        policy.wheres.all? { |field, expected| payload[field] == expected }
      end

      def deliver(policy, event, domain)
        # `record` ASSIGNED BEFORE THE for_each BRANCH, not after — both
        # rescue clauses below call `.merge` on it, and a `return` early
        # (the for_each fan-out has no single record of its own; it
        # rescues its OWN failures per-row, below) used to leave `record`
        # unassigned, so a genuine defect inside `deliver_for_each` was
        # caught here only to raise a SECOND, different `NoMethodError`
        # (`undefined method 'merge' for nil`) trying to record the
        # first one. Computing it unconditionally, first, means this
        # method's own defect handling can never fail that second way.
        target = "#{policy.target_domain || domain}::#{policy.trigger_command}"
        record = { policy: policy.name, on: event.name, trigger: target }

        return deliver_for_each(policy, event, domain) if policy.for_each

        if @door.reaction_depth_reached?
          return record.merge(delivered: false,
                              reason: "reaction depth #{@door.max_reaction_depth} reached")
        end

        # with_literals (vendored addition -- see IR::Policy's own
        # comment) merges in AFTER the event payload, so a policy's own
        # explicit `with "key", "value"` always wins over a same-named
        # payload field -- the literal was written to be authoritative,
        # not a fallback.
        args = event.payload.transform_keys(&:to_sym).merge((policy.with_literals || {}).transform_keys(&:to_sym))
        @door.reenter(target, **args)
        record.merge(delivered: true)
      rescue *DOMAIN_REFUSALS => error
        # The target refused — a fact about the domain, recorded and not
        # fatal to the command that emitted the event.
        record.merge(delivered: false, reason: error.message)
      rescue StandardError => error
        # A DEFECT, not a refusal — a NoMethodError in an interpreter, a
        # NameError from a missing constant, a TypeError from a bad
        # assumption : exactly the class of thing DOMAIN_REFUSALS
        # (errors.rb, see the comment above that constant) deliberately
        # excludes, and for the reason that comment gives at length —
        # folding a crash into the same `delivered: false` shape as an
        # ordinary refusal makes a broken runtime read as normal operation
        # in the log. This clause does not reopen that hole: it is a
        # SECOND, narrower rescue, tried only once the first one above has
        # already declined to match, so a legitimate refusal still takes
        # the branch above and a defect always takes this one.
        #
        # Catching it HERE is safe for a fact this method's caller cannot
        # see from where it sits: by the time `react` runs, the command
        # that EMITTED `event` has already succeeded and PERSISTED —
        # `Dispatcher#dispatch` calls `@policies.react` only after its own
        # `announced` events are already in hand. Letting this exception
        # keep propagating would not undo that command (nothing here is
        # transactional across aggregates) — it would only blow up the
        # ORIGINAL caller's `dispatch` call for a failure that happened in
        # a DIFFERENT command, one the caller never asked to run and has no
        # way to compensate for. So the defect is recorded, distinguishably
        # (`defect: true`, plus the error's own class — nothing here is
        # allowed to read like an ordinary refusal), warned to STDERR so it
        # is never silent, and left exactly where it happened for a human
        # to find — never re-raised, and never swallowed either.
        warn "[hecksagain] defect in reaction — policy #{policy.name} on #{event.name} " \
             "firing #{target}: #{error.class}: #{error.message}"
        record.merge(delivered: false, reason: error.message, defect: true, error_class: error.class.name)
      end

      # `for_each from: "Aggregate.query_name", where: {...}` -- vendored
      # addition, not (yet) upstream hecksagain (migration plan task 8):
      # see PolicyBuilder#for_each's own comment. Runs the named query
      # (within the POLICY's OWN domain -- `trigger`'s own
      # target_domain-or-domain convention, mirrored here since a
      # for_each source and its trigger are the same saga step) with
      # `where:`'s Symbol values resolved against the triggering event's
      # payload, then dispatches the trigger command ONCE PER ROW using
      # the universal `id=<value>` self-ref fallback every aggregate's
      # own self-referencing command already accepts -- no need to know
      # the target command's exact reference-argument name.
      def deliver_for_each(policy, event, domain)
        spec = policy.for_each
        aggregate_name, query_name = spec[:from].split(".", 2)
        # THE SAME target_domain-OR-domain CONVENTION `trigger` USES,
        # applied here too — a for_each source and its trigger are the
        # same saga step (PolicyBuilder#for_each's own comment), so the
        # query resolves against whichever domain the trigger will fire
        # into, not unconditionally the event's own domain.
        source_domain = policy.target_domain || domain
        aggregate = resolve_for_each_aggregate(source_domain, aggregate_name, spec[:from])
        payload = event.payload.transform_keys(&:to_sym)
        resolved_where = (spec[:where] || {}).transform_values do |v|
          v.is_a?(Symbol) ? payload[v] : v
        end

        rows = QueryInterpreter.new(@registry).call(source_domain, aggregate, query_name, resolved_where)
        target = "#{source_domain}::#{policy.trigger_command}"
        literals = (policy.with_literals || {}).transform_keys(&:to_sym)

        Array(rows).map do |row|
          record = { policy: policy.name, on: event.name, trigger: target, for_row: row[:id] }
          if @door.reaction_depth_reached?
            next record.merge(delivered: false,
                              reason: "reaction depth #{@door.max_reaction_depth} reached")
          end

          begin
            @door.reenter(target, id: row[:id], **literals)
            record.merge(delivered: true)
          rescue *DOMAIN_REFUSALS => error
            record.merge(delivered: false, reason: error.message)
          end
        end
      end

      # THE SAME RESOLUTION `Dispatcher#resolve_aggregate` performs — a
      # bare aggregate NAME split out of `for_each from:` is not what
      # `QueryInterpreter#call` needs: `declared_query` calls `.query` on
      # whatever it is handed, and a String has no such method. Mirrored
      # here rather than reused (that method is private on Dispatcher, and
      # this interpreter only ever sees it as `@door`) — an UnknownVerb
      # refusal, the same class `resolve_aggregate` raises, so a for_each
      # naming a domain or aggregate that is not loaded is recorded as an
      # undelivered reaction, not a defect (see DOMAIN_REFUSALS).
      def resolve_for_each_aggregate(domain, aggregate_name, verb)
        bluebook = @registry.bluebook(domain) ||
                   raise(UnknownVerb, RefusalWording.render("UnknownVerb", "no_domain", domain: domain.inspect, verb: verb))
        bluebook.aggregate(aggregate_name) ||
          raise(UnknownVerb, RefusalWording.render("UnknownVerb", "no_aggregate",
                                                    domain: domain, aggregate: aggregate_name.inspect))
      end
    end
  end
end
