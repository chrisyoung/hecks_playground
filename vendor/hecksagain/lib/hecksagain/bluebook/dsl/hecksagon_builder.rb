module Hecksagain
  module Bluebook
    module DSL
      class HecksagonBuilder
        class << self
          attr_accessor :collector

          # The Bind a `success`/`failure` call (inside the block a
          # `charged_by`-shaped verb call opens) writes onto — set for the
          # duration of that block by `BindingProxy#method_missing`/
          # `HecksagonBuilder#method_missing`, same save/restore shape
          # `.collector` already uses. `self` inside that block stays the
          # HecksagonBuilder instance (the whole `Hecks.hecksagon do ...
          # end` chain is one `instance_eval`, and `block&.call` never
          # rebinds it), so `success`/`failure` land here as real instance
          # methods rather than falling into `method_missing` and minting
          # a second, spurious Bind — which is exactly what happened
          # before this existed.
          attr_accessor :current_bind
        end

        attr_reader :binds, :subscriptions, :framework_members, :vendored_bluebooks, :driven_handlers,
                    :driving_handlers

        def initialize(domain)
          @domain             = domain
          @binds              = []
          @subscriptions      = []
          @framework_members  = []
          @vendored_bluebooks = []
          @driven_handlers    = []
          @driving_handlers   = []
        end

        # THE EFFECT-PORT ASYNC VERDICT, captured onto whichever Bind is
        # currently open (see .current_bind, above) — `nil` outside a
        # bind's own block is a silent no-op rather than a raise, since a
        # stray `success`/`failure` at the top level of a hecksagon is a
        # writing mistake this method has no way to attribute to anything.
        def success(command) = current_bind&.success = command.to_s
        def failure(command) = current_bind&.failure = command.to_s

        def current_bind = self.class.current_bind

        # THE DRIVEN SIDE — `adapter "X" do driven on "Domain::Aggregate
        # .Event" do dispatch "Domain::Aggregate.Command", field:
        # "{event_field}" ; success "..." ; failure "..." end end`. Unlike
        # a Bind, a driven handler is a direct in-process cross-context
        # call — no port, no family, no adapter contract — so it is built
        # by its own small capturing DSL (DrivingAdapterBuilder) and
        # accumulated here rather than going through `@binds`.
        #
        # `**_opts` (swallowed, unread) absorbs legacy kwargs-form calls
        # this corpus still has -- `adapter :web_tool, command: "...",
        # tool: :web_fetch, result_into: "..."` / `adapter :fs, root:
        # "..."` / `adapter :shell, name:, command:, args:, output_format:,
        # timeout:`. Those forms predate this driven/driving grammar and
        # named a genuinely different, still-unbuilt custom-adapter-config
        # capability (no builder anywhere reads `root:`/`command:`/etc
        # off an adapter call) -- accepting and discarding them turns a
        # boot-time ArgumentError into the same silent no-op a bare
        # `adapter :symbol` already is, rather than inventing semantics
        # for kwargs nothing consumes. Designing that capability for real
        # is separate, unstarted work.
        def adapter(name, **_opts, &block)
          built = DrivingAdapterBuilder.build(name, &block)
          @driven_handlers.concat(built.driven_handlers)
          @driving_handlers.concat(built.driving_handlers)
          self
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

        # A VENDORED, EXTERNAL bluebook this domain wants attached — same
        # wiring-decision shape `uses_framework` already is, one level
        # further out: not a member shipped inside hecksagain's own lib/,
        # but a separate package (embryonaut_bluebooks) vendored into THIS
        # project's own checkout. See EmbryonautBluebook's own header for
        # the full reasoning on why its ROOT can't be a fixed constant the
        # way Framework::ROOT is.
        #
        # RECORDED ONTO @vendored_bluebooks, same shape `uses_framework`
        # already gives @framework_members — the language's own
        # conformance suite (syntax_conformance_spec.rb's "names what each
        # argument fills, except where nothing can") refuses a keyword
        # argument that lands nowhere, the exact "silent decoration"
        # this codebase's own philosophy refuses everywhere else. A
        # SEPARATE list from framework_members on purpose — that one is
        # load-bearing for a real check (Registry::Verification's
        # `verify_governed_roles!` asks whether "Governance" is in the
        # domain's MERGED framework_members, across every hecksagon block
        # for it); conflating the two would make a vendored bluebook
        # attachment satisfy a Governance check it has nothing to do with.
        def uses_embryonaut_bluebook(name)
          @vendored_bluebooks << name.to_s
          Hecksagain::EmbryonautBluebook.load!(name)
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
          # `verb`-shaped port is a plain `Port`, registered the same
          # way `Hecks.port`'s top-level method already does, not attached
          # to this bluebook's own IR the way an operations-shaped
          # `DomainPort` is.
          return Hecksagain.current_registry.add_port(built) if built.is_a?(Port)

          bluebook_ir.add_port(built)
        end

        # NO refuse_ungoverned_roles! CALL HERE ANY MORE — moved to
        # Registry::Verification#verify_governed_roles! (see that method's
        # own header for why). This builder only ever sees ONE
        # `Hecks.hecksagon "X" do ... end` block's own binds/framework
        # members; a domain split across multiple files (base +
        # environment overlay, `Hecks.boot(path, environment: ...)`) would
        # have every block but the one declaring `uses_framework
        # "Governance"` refused here, even though `Registry#add_hecksagon`
        # merges them into one Hecksagon before anything dispatches
        # against it. Checking the MERGED result once, at verify! time —
        # after every file for this domain has loaded — is both more
        # permissive (no need to repeat `uses_framework` in every file)
        # and strictly more correct (a check against an incomplete,
        # not-yet-merged hecksagon can never see the real final shape).
        def build
          Hecksagon.new(domain: @domain, binds: @binds, subscriptions: @subscriptions,
                         framework_members: @framework_members, vendored_bluebooks: @vendored_bluebooks,
                         driven_handlers: @driven_handlers, driving_handlers: @driving_handlers)
        end

        # DOMAIN-LEVEL DEFAULT BINDS — `persisted_by "Heki"` bare, at the top
        # of a hecksagon block, applies to every aggregate in this domain
        # that doesn't declare its own override. Mirrors `BindingProxy`'s own
        # `method_missing` one level down (`aggregate:` filled in there,
        # `nil` here) — generic over verb name, not hardcoded to
        # `persisted_by`/`projected_by` specifically, so any future verb
        # gets a domain-level default for free too. See `Hecksagon#bind_for`
        # for the fallback lookup this feeds.
        def method_missing(verb, *args, **kwargs, &block)
          return super unless args.first

          bind = Bind.new(aggregate: nil, verb: verb.to_s, adapter: args.first.to_s, role: kwargs[:role]&.to_s,
                           on: kwargs[:on]&.to_s)
          @binds << bind

          previous_bind        = self.class.current_bind
          self.class.current_bind = bind
          begin
            block&.call
          ensure
            self.class.current_bind = previous_bind
          end
          self
        end

        def respond_to_missing?(_name, _include_private = false) = true

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
