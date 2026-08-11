require_relative "../naming"
require_relative "errors"
require_relative "query_interpreter"


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
        return deliver_for_each(policy, event, domain) if policy.for_each

        target = "#{policy.target_domain || domain}::#{policy.trigger_command}"
        record = { policy: policy.name, on: event.name, trigger: target }

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
        # fatal to the command that emitted the event. Anything else (a
        # NoMethodError in an interpreter, an unknown verb in the policy's
        # own declaration) is a DEFECT and flies : it used to land here too,
        # as `delivered: false` beside every legitimate refusal, which made
        # a crashed runtime read as normal operation.
        record.merge(delivered: false, reason: error.message)
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
        payload = event.payload.transform_keys(&:to_sym)
        resolved_where = (spec[:where] || {}).transform_values do |v|
          v.is_a?(Symbol) ? payload[v] : v
        end

        rows = QueryInterpreter.new(@registry).call(domain, aggregate_name, query_name, resolved_where)
        target = "#{policy.target_domain || domain}::#{policy.trigger_command}"
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
    end
  end
end
