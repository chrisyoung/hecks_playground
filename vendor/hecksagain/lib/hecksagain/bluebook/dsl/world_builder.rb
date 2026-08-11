module Hecksagain
  module Bluebook
    module DSL
      class SettingsCollector
        def initialize = @values = {}

        def method_missing(key, *args, &_block)
          @values[key.to_sym] = args.size == 1 ? args.first : args
        end

        def respond_to_missing?(_name, _include_private = false) = true

        def to_h = @values
      end

      # Vendored addition, not (yet) upstream hecksagain (migration plan
      # task 4): the AGGREGATE-QUALIFIED world-binding mirror form —
      # `Pizzas::Order.charged_by("Stripe") do ... end`, exactly the shape
      # pizzas.world itself (the canonical example) uses to mirror its own
      # hecksagon's bind line. Before this patch, `WorldBuilder` had no
      # `ConstShim` resolver at all (unlike `HecksagonBuilder`/
      # `BluebookBuilder`, which both wrap their block eval in one) — so
      # `Pizzas` failed to resolve as a constant during a `.world` block's
      # `instance_eval`, raising `NameError: uninitialized constant Pizzas`
      # for EVERY world file using this form, canonical example included.
      # Confirmed real, not theoretical: found live via miette's
      # voice.world (`Voice::Voice.voiced_by("ElevenLabs") do ... end`).
      #
      # `IR::World#for_verb` / `#for_binding` key purely by verb and
      # adapter name (`@settings["charged_by"]` / `@settings["charged_by:
      # stripe"]`) — the aggregate qualifier is NEVER read back out, it
      # exists only so a `.world` file visually mirrors its `.hecksagon`
      # sibling. So the fix does not need a real per-aggregate binding
      # object the way `BindingProxy` builds for hecksagon (which records
      # aggregate-scoped `IR::Bind`s that a bind lookup DOES key by
      # aggregate) — it only needs the qualified constant chain to resolve
      # to something whose verb calls land in the SAME `@settings` write
      # path the bare top-level form already uses. TODO upstream via
      # bin/evolve.
      class WorldConstProxy
        def self.namespace(domain, builder)
          Module.new do
            define_singleton_method(:const_missing) { |aggregate| WorldConstProxy.new("#{domain}::#{aggregate}", builder) }
          end
        end

        def initialize(fqn, builder)
          @fqn     = fqn
          @builder = builder
        end

        def method_missing(verb, *args, **kwargs, &block)
          @builder.send(:record_binding, verb, *args, **kwargs, &block)
          self
        end

        def respond_to_missing?(_name, _include_private = false) = true
        def to_s = @fqn
      end

      class WorldBuilder
        def initialize(domain)
          @domain   = domain
          @settings = {}
        end

        def realm(value)
          @realm = required(value, "realm")
        end

        def latest(value)
          @latest = required(value, "latest version")
        end

        def method_missing(verb, *args, **kwargs, &block) = record_binding(verb, *args, **kwargs, &block)

        def respond_to_missing?(_name, _include_private = false) = true

        def build
          MetaValidator.call_world(
            IR::World.new(domain: @domain, realm: @realm, latest: @latest, settings: @settings)
          )
        end

        def self.build(domain, &block)
          builder  = new(domain)
          resolver = ->(name) { WorldConstProxy.namespace(name, builder) }

          ConstShim.with(resolver) { builder.instance_eval(&block) } if block
          builder.build
        end

        protected

        # Shared by the bare top-level verb form (`method_missing` above)
        # AND `WorldConstProxy` (the aggregate-qualified mirror form) — one
        # write path, two spellings in.
        def record_binding(verb, *args, **kwargs, &block)
          collector = SettingsCollector.new
          collector.instance_eval(&block) if block
          value = { adapter: args.first.to_s }.merge(kwargs).merge(collector.to_h)
          @settings[verb.to_s] = value
          @settings["#{verb}:#{args.first.to_s.downcase}"] = value
        end

        private

        def required(value, label)
          # moved to the language: Realm / Latest invariants, in world.bluebook
          value.to_s
        end
      end
    end
  end
end
