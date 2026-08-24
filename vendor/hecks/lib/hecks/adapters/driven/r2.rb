require "json"
require "securerandom"

require_relative "r2/connection"
require_relative "heki"
require_relative "../../ports/persistence/append_only"
require_relative "../../ports/query/in_memory"
require_relative "in_memory_ordering"
require_relative "../../runtime/errors"
require_relative "../../runtime/instance"

module Hecks
  module Adapters
    # Cloudflare R2 — the same HEKI wire format Heki writes to a local file
    # (magic-header framing around deflate-compressed, id-sorted JSON;
    # `Heki::Snapshot.encode`/`.decode`, heki/snapshot.rb, called directly
    # below), but PUT/GET over R2's S3-compatible REST API instead of
    # `File.binwrite`/`binread`. Sibling on the Ruby side of
    # `rust/src/heki_r2.rs` on the fork's Rust side — same bytes on the
    # wire, this file is the auth+transport half `heki_r2.rs` gets for
    # free from the Cloudflare Worker's own `Bucket` binding, hand-built
    # here with real SigV4 (see r2/signer.rb, r2/connection.rb).
    #
    # TWO R2 OBJECTS PER AGGREGATE, mirroring the split Heki keeps as two
    # local files (a snapshot + a journal) and D1 keeps as two SQL tables
    # (the aggregate table + its own `_entries` table) — same shape,
    # different transport each time:
    #   "<name>.heki"          — current-state store, upsert-by-id.
    #                            Matches heki_r2.rs's `upsert_record`.
    #   "<name>.entries.heki"  — append-only history, fresh id per entry,
    #                            never updates an existing one. Matches
    #                            heki_r2.rs's own `append_record`.
    # Both objects use the identical HEKI wire codec — only their content
    # differs (current fields vs. an operation/state/mirrors envelope).
    #
    # Every write is a full GET-mutate-PUT of the object it targets — R2
    # has no cheap partial append the way a local file does, so there is
    # no local durability gap for a journal to protect against the way
    # Heki's journal-before-snapshot ordering does: an R2 PUT lands whole
    # or not at all, R2's own guarantee, so `append` and `project` differ
    # only in WHICH object they round-trip, not in when it is safe to
    # trust what landed.
    class R2
      attr_reader :aggregate

      def initialize(aggregate:, settings: {}, root: nil)
        @aggregate = aggregate

        bucket     = settings[:bucket]     || settings["bucket"]
        account    = settings[:account]    || settings["account"]
        access_key = settings[:access_key] || settings["access_key"]
        secret_key = settings[:secret_key] || settings["secret_key"]
        { "bucket" => bucket, "account" => account, "access_key" => access_key, "secret_key" => secret_key }.each do |name, value|
          raise Runtime::WiringError, "R2 needs a #{name.inspect} in its world settings" if value.to_s.empty?
        end

        @connection  = Connection.new(account: account, bucket: bucket, access_key: access_key, secret_key: secret_key)
        @state_key   = "#{@aggregate.storage_name}.heki"
        @entries_key = "#{@aggregate.storage_name}.entries.heki"
      end

      def find(id)
        record = state_store[id.to_s]
        return nil unless record

        instance(id.to_s, record)
      end

      def all(order_by: nil, direction: :asc)
        records = state_store.sort_by { |id, _| id }.map { |id, record| instance(id, record) }
        InMemoryOrdering.ordered(records, aggregate: @aggregate, order_by: order_by, direction: direction)
      end

      def count = state_store.size

      def query(specification, args = {}, context: {})
        Ports::Query::InMemory.execute(all, specification, args)
      end

      # Fresh id per call, mirroring heki_r2.rs's `append_record` exactly
      # — the entry's own identity in the log, distinct from
      # `entry.id` (the aggregate id), which rides along as a value.
      # Ordered by an explicit integer `sequence`, not by timestamp — an
      # R2 object has no autoincrement column the way D1's own `_entries`
      # table does (`sequence` there is real SQL identity), and two
      # appends inside the same wall-clock second are common enough
      # (a save immediately followed by a second save, this adapter's
      # own spec included) that a string timestamp cannot be trusted to
      # order them.
      def append(entry)
        store = read_store(@entries_key)
        log_id = SecureRandom.uuid
        sequence = store.values.map { |record| record["sequence"].to_i }.max.to_i + 1
        store[log_id] = {
          "sequence" => sequence, "aggregate_id" => entry.id,
          "operation" => entry.operation, "state" => entry.state, "mirrors" => entry.mirrors
        }
        write_store(@entries_key, store)
        entry
      end

      # Upsert-by-id against the state store — matches heki_r2.rs's
      # `upsert_record` Rule 1 (explicit id already carried on `entry`).
      def project(entry)
        store = state_store
        entry.delete? ? store.delete(entry.id) : store[entry.id] = entry.state.dup
        write_store(@state_key, store)
        entry
      end

      def save(instance)
        entry = Ports::Persistence::Entry.new(operation: "save", id: instance.id.to_s, state: instance.state.dup)
        append(entry)
        project(entry)
        instance
      end

      def delete(id)
        return false unless find(id)

        entry = Ports::Persistence::Entry.new(operation: "delete", id: id.to_s, state: nil)
        append(entry)
        project(entry)
        true
      end

      def entries
        read_store(@entries_key).values.sort_by { |record| record["sequence"].to_i }.map do |record|
          Ports::Persistence::Entry.new(
            operation: record.fetch("operation"), id: record.fetch("aggregate_id"),
            state: record["state"]&.transform_keys(&:to_sym), mirrors: record["mirrors"]
          )
        end
      end

      # In-memory only, same as Heki's own `record_event`/`events` — R2
      # (like the local filesystem Heki targets) has no event-log object
      # of its own here; heki_r2.rs doesn't define one either.
      def record_event(event) = @events << event
      def events = @events ||= []

      def reset!
        write_store(@state_key, {})
        write_store(@entries_key, {})
        self
      end

      private

      def instance(id, record)
        Runtime::Instance.new(aggregate: @aggregate, id: id, state: record.transform_keys(&:to_sym))
      end

      # Re-fetched every call, deliberately not memoized — same reasoning
      # as D1's own `all`/`find`: a warm long-lived adapter instance must
      # never serve state from before a write another process just made.
      def state_store = read_store(@state_key)

      def read_store(key)
        body = @connection.get(key)
        (body.nil? || body.empty?) ? {} : Heki::Snapshot.decode(body)
      end

      def write_store(key, records)
        @connection.put(key, Heki::Snapshot.encode(records))
      end
    end
  end
end
