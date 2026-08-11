module Hecksagain
  module Bluebook
    module DSL
      class HecksagonBuilder
        class << self
          attr_accessor :collector
        end

        attr_reader :binds, :subscriptions, :framework_members, :raw_adapters, :driving_handlers

        def initialize(domain)
          @domain             = domain
          @binds              = []
          @subscriptions      = []
          @framework_members  = []
          @raw_adapters       = []
          @driving_handlers   = []
        end

        # Vendored addition, not (yet) upstream hecksagain (task 7 of the
        # migration plan): `adapter "Name" do driving on <kind> "<arg>"
        # do |signal| dispatch "Domain::Aggregate.Command" end end` --
        # the DRIVING-SIDE construct (an external clock/file-watch/
        # http-post reaches IN, the inverse of persisted_by/charged_by's
        # driven-side). Confirmed real, live, 32 files in hecks_conception
        # (cron_adapter.hecksagon, agent_inbox.hecksagon,
        # event_sourcing.hecksagon, ...), grammar already proven in TWO
        # other Hecks codebases (rust/src/hecksagon_parser.rs::
        # parse_driving_handler, ruby/hecksagon/dsl/driven_adapter_builder.rb)
        # -- ported here, not invented. STRUCTURAL support only: the
        # actual clock that fires these on a schedule is a SEPARATE
        # concern (Part 1 item 8 of the plan -- keep the old Rust
        # driving-loop process alive, targeting hecksagain-cli's
        # dispatch subcommand, until a Ruby scheduler is built) --
        # documented, not silently pretended complete.
        #
        # ALSO still handles the OLD `adapter :symbol, key: val, ...`
        # form (67+ files) when called with no block -- same method,
        # dispatches on block presence.
        # Vendored addition, not (yet) upstream hecksagain (migration plan
        # task 8): `:sqlite` alongside the original `:heki`/`:memory`
        # domain-wide-default kinds -- the CANONICAL PIZZAS EXAMPLE itself
        # (pizzeria/domain/pizzas.hecksagon) writes `adapter :sqlite, db:
        # "..." ` as a context-wide default with its own documented
        # PRECEDENCE rule ("an aggregate-level persisted_by(...) binding
        # is applied LAST and would OVERRIDE this wiring — exactly how
        # Cart stays in Memory"). Unlike heki/memory, sqlite is written
        # WITH opts (a real inline db path, not a .world-file value), so
        # the trigger condition widens to tolerate opts for a KNOWN
        # backend kind -- opts are accepted structurally but not yet
        # wired to real per-deployment config (a documented, narrower
        # gap: inline hecksagon-level adapter config bypassing .world
        # entirely is a genuinely separate feature this migration didn't
        # build). DOMAIN_WIDE_KINDS maps the DSL's bare kind symbol to
        # its real registered adapter name (Sqlite's own .adapter file
        # names it "SqlitePersistence", not "Sqlite").
        DOMAIN_WIDE_KINDS = { heki: "Heki", memory: "Memory", sqlite: "SqlitePersistence" }.freeze

        def adapter(kind, **opts, &block)
          return domain_wide_persisted_by(kind) if !block && DOMAIN_WIDE_KINDS.key?(kind)
          return @raw_adapters << { kind: kind.to_s, opts: opts } unless block

          @driving_handlers.concat(DrivingAdapterBuilder.build(kind, &block))
        end

        # Vendored addition, not (yet) upstream hecksagain: `adapter
        # :heki` / `adapter :memory` / `adapter :sqlite` bare (no
        # aggregate qualifier, no block) is a DOMAIN-WIDE default --
        # "every aggregate in this bluebook persists here unless
        # overridden" -- widespread across miette's organ/memory domains
        # (i728 Phase B, "durable-by-default") and the canonical pizzas
        # example's sqlite context, which states the precedence
        # explicitly: "an aggregate-level persisted_by(...) binding is
        # applied LAST and would OVERRIDE this wiring." hecksagain has no
        # domain-wide default; every aggregate needs its own explicit
        # persisted_by bind.
        #
        # DEFERRED, not immediate -- pizzeria's own file declares the
        # domain-wide `adapter :sqlite` FIRST and Cart's overriding
        # `Cart.persisted_by("Memory")` AFTER it, so resolving the
        # default immediately (as this used to) saw an empty @binds and
        # synthesized a bind for Cart too, colliding with Cart's own
        # explicit one two lines later ("2 authoritative persisted_by
        # bindings"). Recorded here, applied once in #build once every
        # explicit bind in THIS FILE is known -- scoped to this
        # hecksagon file only, the known, narrower gap a multi-file-per-
        # domain override in a DIFFERENT file wouldn't be caught by ;
        # pizzeria is single-file, so this is exact there. TODO upstream
        # via bin/evolve (migration plan task 7).
        def domain_wide_persisted_by(kind)
          (@domain_wide_defaults ||= []) << DOMAIN_WIDE_KINDS.fetch(kind)
        end

        def apply_domain_wide_defaults!
          return if @domain_wide_defaults.nil? || @domain_wide_defaults.empty?

          bluebook_ir = Hecksagain.current_registry.bluebook(@domain) or return
          already_bound = @binds.select { |b| b.verb == "persisted_by" }.map(&:aggregate_name).to_set
          @domain_wide_defaults.each do |backend|
            bluebook_ir.aggregates.each do |agg|
              next if already_bound.include?(agg.hecks_name)

              @binds << IR::Bind.new(aggregate: agg.hecks_name, verb: "persisted_by", adapter: backend, role: nil)
              already_bound << agg.hecks_name
            end
          end
        end

        # An event this hecksagon takes from OUTSIDE the domain's own
        # bluebook.
        def subscribe(event) = @subscriptions << event.to_s

        # A framework/ member this domain wants attached —
        # Governance, Identity, whatever else lands beside them.
        # Attaching one is a WIRING decision, the same kind `persisted_by`/
        # `projected_by` already are, so it lives here rather than as a
        # fact stated in the domain's own bluebook. Recorded onto THIS
        # hecksagon, the same way `subscribe` records onto its own
        # `subscriptions` — and loads the member's bluebook then its own
        # hecksagon into whatever registry this one is loading into, see
        # `Framework.load!`.
        def uses_framework(name)
          @framework_members << name.to_s
          Hecksagain::Framework.load!(name)
        end

        # THE PRIMARY PORT, BARE AT THE ROOT — belongs to the CHAPTER as a
        # whole, not one aggregate. `BindingProxy#port` is the aggregate-
        # scoped sibling (`Payments::Payment.port("Gateway") do ... end`);
        # this is what's left when a port isn't about any one record. The
        # bluebook must already be built and registered, since a hecksagon
        # loads after its bluebook, and this attaches to that real, final
        # object directly rather than building a second copy MetaValidator
        # would have to know how to reconstruct.
        def port(name, &block)
          bluebook_ir = Hecksagain.current_registry.bluebook(@domain) or
            raise Malformed, "#{@domain} declares no such bluebook — a port needs one to belong to"

          # See BindingProxy#port's own comment on why this resolver swap is
          # needed — ConstShim's active resolver is one global for the whole
          # dynamic extent, currently this file's own BindingProxy-minting
          # one, which would turn a bare constant inside an operation's
          # `reference_to`/`attribute` into another BindingProxy instead of
          # a name.
          built = ConstShim.with(->(const) { const }) { DomainPortBuilder.build(name, &block) }

          # See BindingProxy#port's own comment on the same branch — a
          # `verb`-shaped port is a plain `IR::Port`, registered the same
          # way `Hecks.port`'s top-level method already does, not attached
          # to this bluebook's own IR the way an operations-shaped
          # `IR::DomainPort` is.
          return Hecksagain.current_registry.add_port(built) if built.is_a?(IR::Port)

          bluebook_ir.add_port(built)
        end

        # Vendored no-op stub, not (yet) upstream hecksagain: hecksagon-
        # level `gate "Aggregate", :role do allow :Cmd1, :Cmd2, ... end`
        # (found in miette's circuit_breaker.hecksagon). Investigated, not
        # just stubbed blind: every command in circuit_breaker.bluebook
        # already declares its OWN `role "..."` individually, which
        # hecksagain's `Runtime.as_caller(role:)` already checks at
        # dispatch time — so `gate`/`allow` looks like a duplicate,
        # centralized restatement of the same authorization from an older
        # convention, not new capability. It's also STALE where checked:
        # the bluebook says `Trip`/`Reset` are role "Creator", but this
        # gate block claims :system covers them too — a real disagreement,
        # not just redundancy, which is one more reason not to silently
        # promote it to enforcement without reconciling the conflict
        # first. Accepted so the file boots ; not stored or enforced.
        # TODO upstream via bin/evolve (migration plan task 7): decide
        # whether `gate` is retired in favor of per-command `role`, or
        # kept as an intentional stricter allowlist — and if kept,
        # reconcile circuit_breaker's own conflicting statement.
        class GateStub
          def allow(*) = nil
        end

        def gate(*, &block)
          GateStub.new.instance_eval(&block) if block
        end

        # Vendored no-op stub, not (yet) upstream hecksagain — and a
        # MAJOR finding, not a small one (migration plan task 4): the
        # `Aggregate.verb("Adapter", on: "Event") do success "Cmd"
        # failure "Cmd" end` shape — the EFFECT-family async-verdict
        # pattern the Pizzas example itself documents as canonical
        # (`Order.charged_by("Stripe", on: "OrderPlaced") do success
        # "Order.Authorize" failure "Order.Decline" end`) — has NO
        # implementation ANYWHERE in hecksagain: confirmed by grepping
        # both the vendored copy AND the untouched original
        # ~/Projects/hecksagain repo for `def success`/`def failure` —
        # zero hits in either. `IR::Bind` itself
        # (bluebook/ir/hexagon.rb) has no `on`/`success`/`failure`
        # fields at all, only `aggregate`/`verb`/`adapter`/`role`. Found
        # live via miette's dream.hecksagon (`BodyDream::Dream.
        # imaged_by("DreamImage", on: "DreamImageRequested") do success
        # "Dream.RecordImage" end`) — a block whose `success`/`failure`
        # calls land on THIS builder (BindingProxy#method_missing just
        # `block&.call`s the block in its ORIGINAL lexical scope, never
        # instance_eval's it against the bind), so they need to exist
        # here regardless of who resolves them.
        #
        # Stubbed structurally so the file boots — NOT a real fix. A
        # real fix means: `on:` threaded onto IR::Bind, an actual async
        # delivery mechanism (there is no adapter-host process in this
        # runtime the way the Pizzas narrative implies), and genuine
        # verdict re-entry wiring (a refused/succeeded response
        # dispatching the named command back in). That is a whole
        # subsystem, not a missing keyword — deliberately not attempted
        # under this session's time pressure. TODO upstream via
        # bin/evolve (migration plan task 7): this is the single
        # highest-value remaining gap this migration surfaced.
        def success(*) = nil
        def failure(*) = nil

        # Vendored catch-all no-op, not (yet) upstream hecksagain
        # (migration plan task 8): bin-buddy's portal `.hecksagon` files
        # (driver_portal/admin_portal/homeowner_portal) are a genuinely
        # DIFFERENT genre of content from aggregate-wiring — app-surface
        # branding and UI config (`capabilities :webapp`, `brand_color
        # "#..."`, `task_colors do out "#..." end`, `refund_thresholds do
        # role :support, cents: 2500 end`) — not persistence/payment
        # binds at all. Real methods (`adapter`/`success`/`failure`/etc
        # above) still win over method_missing, so this only absorbs
        # whatever this vendored port doesn't know, same pattern as
        # DrivingAdapterBuilder's own catch-all just above it in this
        # migration. A block passed to an absorbed call is never
        # yielded to, so its own inner calls (`out "#..."`, `role :x,
        # cents: n`) never need to exist either — structurally captured,
        # not wired to any real portal/branding concept. TODO upstream
        # via bin/evolve (migration plan task 8).
        def method_missing(*) = nil
        def respond_to_missing?(*) = true

        def build
          apply_domain_wide_defaults!
          IR::Hecksagon.new(domain: @domain, binds: @binds, subscriptions: @subscriptions,
                             framework_members: @framework_members, driving_handlers: @driving_handlers)
        end

        def self.build(domain, &block)
          builder  = new(domain)
          resolver = ->(name) { BindingProxy.namespace(name, builder.binds) }

          previous       = collector
          self.collector = builder.binds
          begin
            ConstShim.with(resolver) { builder.instance_eval(&block) } if block
          ensure
            self.collector = previous
          end

          builder.build
        end
      end
    end
  end
end
