module Hecksagain
  module Bluebook
    module IR
      Port = Struct.new(:name, :verb, :signal, keyword_init: true) do
        def reply?  = signal == :reply
        def effect? = signal == :effect
      end

      Adapter = Struct.new(:name, :port, :fields, :secrets, keyword_init: true) do
        def declares?(field) = all_fields.include?(field.to_sym)

        def all_fields = (fields || []) + (secrets || [])
      end

      Bind = Struct.new(:aggregate, :verb, :adapter, :role, keyword_init: true) do
        def aggregate_name = Naming.demodulise(aggregate)
      end

      # Vendored addition, not (yet) upstream hecksagain (migration plan
      # task 7): a DRIVING-side handler -- an external clock/file-watch/
      # http-post reaching IN to fire a dispatch, the inverse of Bind's
      # driven-side edge. adapter_name names the `adapter "X" do ... end`
      # block it was declared in; kind is "cron"/"interval"/"http_post"/
      # "file_watch"; arg is the schedule/path/route string;
      # dispatch_command is the FQN the handler fires on tick.
      DrivingHandler = Struct.new(:adapter_name, :kind, :arg, :dispatch_command, keyword_init: true) do
        def to_h = { adapter_name: adapter_name, kind: kind, arg: arg, dispatch_command: dispatch_command }
      end

      class Hecksagon
        attr_reader :domain, :binds, :subscriptions, :framework_members, :driving_handlers

        def initialize(domain:, binds: [], subscriptions: [], framework_members: [], driving_handlers: [])
          @domain             = domain.to_s
          @binds              = binds
          @subscriptions      = subscriptions
          @framework_members  = framework_members
          @driving_handlers   = driving_handlers
        end

        def bind_for(aggregate_name, verb)
          @binds.find do |b|
            b.aggregate_name == aggregate_name.to_s && b.verb.to_s == verb.to_s
          end
        end

        def binds_for(aggregate_name, verb)
          @binds.select do |b|
            b.aggregate_name == aggregate_name.to_s && b.verb.to_s == verb.to_s
          end
        end

        def to_h
          { domain: @domain, binds: @binds.map(&:to_h), subscriptions: @subscriptions.map(&:to_s),
            framework_members: @framework_members.map(&:to_s),
            driving_handlers: @driving_handlers.map(&:to_h) }
        end
      end

      class World
        attr_reader :domain, :realm, :latest, :settings

        def initialize(domain:, realm: nil, latest: nil, settings: {})
          @domain   = domain.to_s
          @realm    = realm&.to_s
          @latest   = latest&.to_s
          @settings = settings
        end

        def for_verb(verb) = @settings.fetch(verb.to_s, {})

        # Vendored fix, not (yet) upstream hecksagain (migration plan
        # task 8): the bare fallback used to be `@settings.fetch(
        # "#{verb}:#{adapter}", for_verb(verb))` unconditionally -- but
        # `for_verb(verb)` is whichever adapter's bare top-level block
        # was declared LAST (e.g. `persisted_by("Heki") do dir :default
        # end`), and falling back to it for an UNRELATED adapter applies
        # one adapter's settings to another's bind. Real, corpus-caught
        # bug : deciderate.hecksagon binds Game/Vote to Heki and Bubble
        # to Memory (the Cart-equivalent ephemeral aggregate) ; the
        # world configures only Heki's `dir`, and Bubble's lookup for
        # "persisted_by:memory" (absent, correctly -- Memory carries no
        # values) fell back to Heki's `{adapter: "Heki", dir: ...}`,
        # then failed `check_settings` with "Memory does not declare
        # :dir". Only fall back when the generic entry's OWN adapter
        # actually matches the one being asked about -- otherwise there
        # is genuinely nothing configured for this bind, `{}`, exactly
        # right for an adapter like Memory that takes no values at all.
        def for_binding(verb, adapter)
          qualified = @settings["#{verb}:#{adapter.to_s.downcase}"]
          return qualified if qualified

          generic = for_verb(verb)
          generic[:adapter].to_s.downcase == adapter.to_s.downcase ? generic : {}
        end

        def to_h = { domain: @domain, realm: @realm, latest: @latest, settings: @settings }
      end
    end
  end
end
