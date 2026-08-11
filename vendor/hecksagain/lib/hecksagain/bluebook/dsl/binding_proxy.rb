module Hecksagain
  module Bluebook
    module DSL
      class BindingProxy
        def self.namespace(domain, collector)
          Module.new do
            define_singleton_method(:const_missing) do |aggregate|
              BindingProxy.new("#{domain}::#{aggregate}", collector)
            end
          end
        end

        def initialize(fqn, collector)
          @fqn       = fqn
          @collector = collector
        end

        # THE AGGREGATE-SCOPED PORT — `Payments::Payment.port("Gateway") do
        # ... end`, the same receiver a plain bind like `.persisted_by(...)`
        # already reaches, because a port belongs to exactly one aggregate
        # the same way a bind does. A REAL method, not method_missing : its
        # shape (a name and a block building operations) has nothing to do
        # with `IR::Bind`, so it does not belong in that generic verb path.
        def port(name, &block)
          domain, aggregate_name = @fqn.split("::")
          aggregate_ir = Hecksagain.current_registry.bluebook(domain)&.aggregate(aggregate_name) or
            raise Malformed, "#{@fqn} declares no such aggregate — a port needs one to belong to"

          # See HecksagonBuilder#port's own comment on why this resolver
          # swap is needed : ConstShim's active resolver is one global for
          # the whole dynamic extent, and it is currently this file's own
          # BindingProxy-minting one, which would turn a bare `Pizza` inside
          # `reference_to Pizza` into another BindingProxy instead of a name.
          built = ConstShim.with(->(const) { const }) { DomainPortBuilder.build(name, owner: aggregate_name, &block) }

          # A `verb`-shaped port is a plain `IR::Port` — the exact struct
          # `Hecks.port`'s own top-level method registers, so it goes
          # through the same `add_port` the registry already answers for
          # that call. It belongs to no aggregate IR the way an
          # operations-shaped `IR::DomainPort` does; the aggregate above
          # was only needed to resolve `owner` for the operations branch.
          if built.is_a?(IR::Port)
            Hecksagain.current_registry.add_port(built)
            return self
          end

          built.operations.each do |operation|
            operation.attributes.select(&:reference?).each { |attribute| attribute.type.declared_in = aggregate_ir }
          end
          aggregate_ir.add_port(built)
          self
        end

        # Vendored addition, not (yet) upstream hecksagain (migration plan
        # task 4): `Aggregate.event_sourced` -- a bare MARKER verb, no
        # adapter argument at all (framework/tools/bluebook/tools.
        # hecksagon, framework/hexagon/bluebook/outbound_event.hecksagon:
        # "this aggregate's own event log IS its persistence, no separate
        # backend choice needed"). Every other bind verb needs a real
        # adapter name (`args.first`) -- `event_sourced` structurally
        # never supplies one, so the generic path below would mint an
        # empty-string adapter and fail wiring ("unknown adapter \"\"").
        # Reframed as sugar for `persisted_by("Heki")` -- Heki IS the
        # durable store an event-sourced aggregate's log actually rides
        # on, and the corpus's own bluebook text stays exactly
        # `.event_sourced`, unchanged; only the IR::Bind this produces
        # differs from what the text literally says. TODO upstream via
        # bin/evolve (migration plan task 7): decide whether
        # `event_sourced` deserves its own real persistence-family verb
        # instead of aliasing persisted_by.
        EVENT_SOURCED_ADAPTER = "Heki"

        def method_missing(verb, *args, **kwargs, &block)
          if verb == :event_sourced
            # Additive-only : outbound_event.hecksagon writes BOTH an
            # explicit `persisted_by("Heki")` AND `.event_sourced` on the
            # SAME aggregate -- the author's own "persistence+" comment
            # says event_sourced OVERLAYS an existing bind, not replaces
            # or duplicates it. Skip minting a second persisted_by bind
            # when one already exists for this aggregate ; still mint one
            # when event_sourced is the ONLY persistence statement
            # (tools.hecksagon's FileTool/ShellTool, which have no other
            # bind at all).
            # `aggregate_name` (demodulised), not raw `aggregate` equality
            # -- domain-wide-default sugar (HecksagonBuilder#
            # domain_wide_persisted_by, a bare `adapter :memory`/`:heki`)
            # stores the BARE aggregate name ("ShellTool"), while a
            # BindingProxy-minted bind (this file) stores the full FQN
            # ("Tools::ShellTool") -- raw string equality never matched
            # the domain-wide-sugar bind, so tools.hecksagon's own
            # `adapter :memory` + FileTool/ShellTool's `.event_sourced`
            # combination still doubled up until this normalized.
            already_bound = @collector.any? { |b| b.aggregate_name == Naming.demodulise(@fqn) && b.verb == "persisted_by" }
            unless already_bound
              @collector << IR::Bind.new(
                aggregate: @fqn,
                verb:      "persisted_by",
                adapter:   EVENT_SOURCED_ADAPTER,
                role:      kwargs[:role]&.to_s
              )
            end
            block&.call
            return self
          end

          @collector << IR::Bind.new(
            aggregate: @fqn,
            verb:      verb.to_s,
            adapter:   args.first.to_s,
            role:      kwargs[:role]&.to_s
          )
          block&.call
          self
        end

        def respond_to_missing?(_name, _include_private = false) = true

        def to_s = @fqn
      end
    end
  end
end
