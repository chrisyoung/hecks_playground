# Hecks::Adapters::EventLogWalker
#
# Answers the LogWalker port : the three EventSourcing::Event queries a where()
# cannot say. Pure computation over the Log rows the caller passes in as `log`;
# it reads and writes nothing, so it needs no runtime handle and is the same
# in memory and in production.
#
#   walker = Hecks::Adapters::EventLogWalker.new
#   walker.causation_trace(event_id: { value: "e2" }, log: rows)  # => CausationChain row
#   walker.consequence_tree(event_id: { value: "e1" }, log: rows) # => ConsequenceTree row
#   walker.chain_intact(log: rows)                                # => IntegrityReport row
#
# The rows are plain Hashes shaped like EventSourcing::Event (value objects come
# as { value: x } or as bare scalars ; both are read). The hash recipe for
# ChainIntact is documented on the IntegrityReport value object in the bluebook.
require "digest"

module Hecks
  module Adapters
    class EventLogWalker
      # Backward : the event, then its cause, then that cause's cause, up to the root.
      def causation_trace(event_id:, log:)
        by_id = entries(log).to_h { |e| [e[:event_id], e] }
        start = scalar(event_id)
        steps = []
        seen  = {}
        current = by_id[start]
        stopped = current ? nil : "unknown"
        while current
          seen[current[:event_id]] = true
          steps << step(current, steps.size)
          cause = current[:causation_id]
          if cause.empty? then stopped = "root"
          elsif seen[cause] then stopped = "cycle"
          elsif !by_id[cause] then stopped = "missing"
          end
          break if stopped

          current = by_id[cause]
        end
        { start_event_id: start, root_event_id: steps.empty? ? "" : steps.last[:event_id],
          stopped: stopped, reached_root: stopped == "root", steps: steps }
      end

      # Forward : the event, then everything it caused, breadth-first, siblings in sequence order.
      def consequence_tree(event_id:, log:)
        rows     = entries(log)
        root     = scalar(event_id)
        children = rows.reject { |e| e[:causation_id].empty? }.sort_by { |e| e[:sequence] }.group_by { |e| e[:causation_id] }
        start    = rows.find { |e| e[:event_id] == root }
        steps    = []
        queue    = start ? [[start, 0]] : []
        seen     = {}
        until queue.empty?
          row, depth = queue.shift
          next if seen[row[:event_id]]

          seen[row[:event_id]] = true
          steps << step(row, depth)
          (children[row[:event_id]] || []).each { |child| queue << [child, depth + 1] }
        end
        { root_event_id: root, size: steps.size, height: steps.map { |s| s[:depth] }.max || 0, steps: steps }
      end

      # Per shard, in sequence order : every entry must hash to its entry_hash and name its
      # predecessor's entry_hash (empty for the first) as its prev_hash.
      def chain_intact(log:)
        rows   = entries(log)
        broken = rows.group_by { |e| shard_of(e[:event_id]) }.flat_map do |shard, shard_rows|
          previous = ""
          shard_rows.sort_by { |e| e[:sequence] }.filter_map do |entry|
            reason = if digest(entry) != entry[:entry_hash] then "edited"
                     elsif entry[:prev_hash] != previous then "broken_link"
                     end
            previous = entry[:entry_hash]
            { event_id: entry[:event_id], shard: shard, sequence: entry[:sequence], reason: reason } if reason
          end
        end
        { checked: rows.size, intact: broken.empty?, broken: broken }
      end

      private

      def entries(log) = Array(log).map { |row| flatten(row) }

      def flatten(row)
        { event_id: scalar(at(row, :event_id)), causation_id: scalar(at(row, :causation_id)),
          sequence: scalar(at(row, :sequence)).to_i, aggregate_name: scalar(at(row, :aggregate_name)),
          aggregate_id: scalar(at(row, :aggregate_id)), verb: scalar(at(at(row, :command), :verb)),
          inputs: scalar(at(at(row, :command), :inputs)), event_name: scalar(at(row, :event_name)),
          delta_field: scalar(at(at(row, :delta), :field)), delta_value: scalar(at(at(row, :delta), :value)),
          correlation_id: scalar(at(row, :correlation_id)), actor: scalar(at(row, :actor)),
          recorded_at: scalar(at(row, :recorded_at)), prev_hash: scalar(at(row, :prev_hash)),
          entry_hash: scalar(at(row, :entry_hash)) }
      end

      def at(hash, key)
        return nil unless hash.respond_to?(:key?)

        hash.key?(key) ? hash[key] : hash[key.to_s]
      end

      # A value object arrives as { value: x } or already bare ; nil reads as empty text.
      def scalar(value)
        value = at(value, :value) if value.respond_to?(:key?) && (value.key?(:value) || value.key?("value"))
        value.nil? ? "" : value
      end

      def step(entry, depth)
        { event_id: entry[:event_id], causation_id: entry[:causation_id], aggregate_name: entry[:aggregate_name],
          aggregate_id: entry[:aggregate_id], verb: entry[:verb], event_name: entry[:event_name],
          sequence: entry[:sequence], depth: depth }
      end

      def shard_of(event_id) = event_id.include?(":") ? event_id.split(":", 2).first : ""

      def digest(entry)
        parts = entry.values_at(:event_id, :aggregate_name, :aggregate_id, :verb, :inputs, :event_name,
                                :delta_field, :delta_value, :causation_id, :correlation_id, :actor,
                                :sequence, :recorded_at, :prev_hash)
        Digest::SHA256.hexdigest(parts.join("\x1f"))
      end
    end
  end
end
