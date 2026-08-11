require_relative "registry/verification"

module Hecksagain
  module Runtime
    class WiringError < StandardError; end

    # The collections a boot gathers — bluebooks, hexagons, ports, adapters,
    # worlds, the logs — and how a repository is resolved from them. The
    # wiring gate lives in registry/verification.rb.
    class Registry
      include Verification

      attr_reader :root, :bluebooks, :hecksagons, :ports, :adapters, :worlds, :event_log,
                  :reaction_log, :saga_log, :saga_instances, :translations

      def initialize(root: nil)
        @root         = root
        @bluebooks    = {}
        @hecksagons   = {}
        @ports     = {}
        @adapters     = {}
        @worlds       = {}
        @translations = []
        @event_log    = []
        @reaction_log = []
        @saga_log = []
        @saga_instances = Hash.new { |h, k| h[k] = {} }
        @repositories = {}
        @projection_repositories = {}
        @bluebook_builders = {}
      end

      # THE BUILDER STAYS OPEN FOR THE LIFE OF THIS REGISTRY, keyed by chapter
      # name — see the comment on `BluebookBuilder.build`. A chapter split across
      # several files (`language/bluebook/*.bluebook`, all `Hecks.bluebook "Bluebook"`)
      # needs its declarations to accumulate into ONE builder rather than each
      # file minting its own and silently discarding the one before.
      def bluebook_builder(name)
        @bluebook_builders[name.to_s] ||= yield
      end

      def add_bluebook(item)  = @bluebooks[item.name]  = item

      # Vendored fix, not (yet) upstream hecksagain (migration plan task
      # 4): a plain overwrite before this -- the SAME gap `bluebook_
      # builder` right above already exists to solve for multi-file
      # BLUEBOOKS ("needs its declarations to accumulate into ONE builder
      # rather than each file minting its own and silently discarding the
      # one before"), just never extended to hecksagons. A domain's
      # `.hecksagon` binds routinely split across files one-per-aggregate
      # (conductor/{merge_queue,lease,sweeper,claim,worker}.hecksagon, all
      # `Hecks.hecksagon "Conductor"`) -- each call built a FRESH
      # IR::Hecksagon and overwrote the one before, so only the LAST
      # file's binds survived registry-wide ("Conductor::MergeQueue has
      # no persisted_by bind" even though merge_queue.hecksagon plainly
      # binds it -- lease.hecksagon just loaded after and clobbered it).
      # Merges binds/subscriptions/framework_members/driving_handlers
      # across every call for the same domain instead.
      def add_hecksagon(item)
        existing = @hecksagons[item.domain]
        @hecksagons[item.domain] = existing ? merge_hecksagons(existing, item) : item
      end

      def add_port(item)    = @ports[item.name]   = item
      def add_adapter(item)   = @adapters[item.name]   = item
      def add_world(item)     = @worlds[item.domain]   = item
      def add_translation(item) = @translations << item

      # {domain name => era ordinal} as resolved by the boot-time era
      # gate. A lineage adapter writes into ITS OWN era's partition —
      # which, for an old checkout booting a held-but-superseded shape,
      # is not the newest one.
      def resolved_eras = @resolved_eras ||= {}

      def bluebook(name)  = @bluebooks[name.to_s]
      def hecksagon(name) = @hecksagons[name.to_s]
      def world(name)     = @worlds[name.to_s]

      def verbs = @bluebooks.values.flat_map(&:verbs).sort

      def repository(domain, aggregate)
        @repositories[[domain.to_s, aggregate.hecks_name]] ||= Ports::Persistence.repository(self, domain, aggregate)
      end

      def capability_graph
        @capability_graph ||= CapabilityGraph.new(self)
      end

      def read_repository(domain, aggregate)
        key = [domain.to_s, aggregate.hecks_name]
        binding = Ports::Projection.binds_for(self, domain, aggregate).first
        return repository(domain, aggregate) unless binding

        projection = (@projection_repositories[key] ||= begin
          projection = Ports::Persistence::RepositoryFactory.build(self, domain, aggregate, binding,
                                                                    recover: true, settings_verb: Ports::Projection::VERB)
          projection
        end)
        authoritative = repository(domain, aggregate)
        projection_current?(projection, authoritative) ? projection : authoritative
      end

      # See #add_hecksagon's own comment.
      def merge_hecksagons(a, b)
        Bluebook::IR::Hecksagon.new(
          domain:            a.domain,
          binds:             a.binds + b.binds,
          subscriptions:     a.subscriptions + b.subscriptions,
          framework_members: a.framework_members + b.framework_members,
          driving_handlers:  a.driving_handlers + b.driving_handlers
        )
      end

      def projection_current?(projection, authoritative)
        projected_entries = projection.entries
        source_entries = authoritative.entries
        return false unless projected_entries.length == source_entries.length
        return false unless projected_entries.zip(source_entries).all? do |projected, source|
          projected.operation == source.operation && projected.id == source.id && projected.state == source.state
        end

        projected_rows = projection.all.map(&:to_h).sort_by { |row| row.fetch(:id).to_s }
        source_rows = authoritative.all.map(&:to_h).sort_by { |row| row.fetch(:id).to_s }
        projected_rows == source_rows
      rescue StandardError
        false
      end

    end
  end
end
