require_relative "behaviour/hexagon"
require_relative "../ir"

module Hecksagain
  module Bluebook
    # `produces` — vendored addition, not (yet) upstream hecksagain
    # (parser-removal plan, Phase 1a): the verdict data a conforming
    # effect-family handler must emit on success (payment's :payment_ref,
    # screenshot_buffer's :path, ...). Previously discarded silently at
    # the family/port boundary. Still inert at runtime today (nothing
    # branches on it, same as `signal` before the effect-port work
    # lands), but the IR now keeps the fact instead of dropping it.
    Port = Struct.new(:name, :verb, :signal, :produces, keyword_init: true) do
      def reply?  = signal == :reply
      def effect? = signal == :effect
    end

    # `handler` — the out-of-process program `bin/adapter-host` execs when
    # this adapter's bound event fires (`handler "path/to/program"` in a
    # `.adapter` file). Read by `record_effect_outbound`'s intended
    # consumer, not yet wired in this gem — see Bind's own `on`/`success`/
    # `failure` comment, below.
    Adapter = Struct.new(:name, :port, :fields, :secrets, :handler, keyword_init: true) do
      def declares?(field) = all_fields.include?(field.to_sym)

      def all_fields = (fields || []) + (secrets || [])
    end

    # `on`/`success`/`failure` — the effect-port async verdict pattern
    # (`Order.charged_by("Stripe", on: "OrderPlaced") do success
    # "Order.Authorize" ; failure "Order.Decline" end`). `on` is the
    # triggering event name (bare — "OrderPlaced", not domain-qualified) ;
    # `success`/`failure` are bare or qualified command FQNs, filled in by
    # `BindingProxy#success`/`#failure` at DSL-build time onto the bind
    # `HecksagonBuilder.current_bind` is tracking for the duration of the
    # block. Read by the effect-port producer this gem does not yet carry
    # (`record_effect_outbound` — needs an `OutboundEvent` framework
    # bluebook this gem's own corpus does not declare ; a consuming
    # corpus's own is where that half belongs). The DSL-capture half —
    # `on`/`success`/`failure` actually landing on the right Bind instead
    # of `HecksagonBuilder#method_missing` minting two spurious binds
    # named "success"/"failure" — is real as of this struct.
    Bind = Struct.new(:aggregate, :verb, :adapter, :role, :on, :success, :failure, keyword_init: true) do
      def aggregate_name = Naming.demodulise(aggregate)
    end

    # The DRIVEN side's counterpart to Bind's on:/success/failure :
    # `adapter "X" do driven on "Domain::Aggregate.Event" do dispatch
    # "Domain::Aggregate.Command", field: "{event_field}" ; success "..." ;
    # failure "..." end end`. Unlike Bind (a PORT-mediated effect, async,
    # swappable adapter), a driven handler is a direct in-process
    # cross-context call — no port, no family, no adapter contract — so it
    # lives on the Hecksagon directly, not through `Registry#port_for` the
    # way Bind does. `dispatch_args` carries the `{field}` interpolation
    # map ; `success`/`failure` are optional — a driven block declaring
    # neither is fire-and-forget. `for_each` is the fan-out spec (`{ from:
    # "Domain::Aggregate.query", where: { ... } }`) captured verbatim by
    # DrivenCapture#dispatch and resolved at delivery time by
    # Runtime::Dispatcher#fan_out_driven — nil for the ordinary
    # one-dispatch-per-event handler.
    DrivenHandler = Struct.new(:adapter_name, :event, :dispatch_command, :dispatch_args, :for_each,
                                :success, :failure, keyword_init: true)

    # The DRIVING side — an external clock reaching IN, the inverse of a
    # driven handler's domain-event-out edge. `adapter_name` names the
    # `adapter "X" do ... end` block it was declared in ; `kind` is
    # "cron"/"interval" ; `arg` is the schedule string ; `dispatch_command`
    # is the FQN the handler fires on tick. Wire shape only in this gem —
    # no DSL yet builds one (the `driving on cron/interval` grammar and its
    # runtime scheduler are their own, still-pending unit), so this
    # collection is always empty here. Carried anyway, same reasoning as
    # `Adapter#handler`: the collection existing is what makes
    # `Registry#merge_hecksagons` need to merge it correctly across a
    # multi-file hecksagon RIGHT NOW, before anything ever populates it —
    # waiting until the DSL lands would repeat 1688477's own mistake, which
    # deferred this exact merge until the attribute existed and then had to
    # come back for it.
    DrivingHandler = Struct.new(:adapter_name, :kind, :arg, :dispatch_command, keyword_init: true)

    class Hecksagon
      include Hecksagain::IR
      include Behaviour::Hecksagon

      emits_ir(
        domain:             :domain,
        binds:              many(:binds),
        subscriptions:      -> { subscriptions.map(&:to_s) },
        framework_members:  -> { framework_members.map(&:to_s) },
        vendored_bluebooks: -> { vendored_bluebooks.map(&:to_s) },
        driven_handlers:    many(:driven_handlers),
        driving_handlers:   many(:driving_handlers)
      )

      attr_reader :domain, :binds, :subscriptions, :framework_members, :vendored_bluebooks, :driven_handlers,
                  :driving_handlers

      def initialize(domain:, binds: [], subscriptions: [], framework_members: [], vendored_bluebooks: [],
                      driven_handlers: [], driving_handlers: [])
        @domain             = domain.to_s
        @binds              = binds
        @subscriptions      = subscriptions
        @framework_members  = framework_members
        @vendored_bluebooks = vendored_bluebooks
        @driven_handlers    = driven_handlers
        @driving_handlers   = driving_handlers
      end


    end

    class World
      include Hecksagain::IR
      include Behaviour::World

      emits_ir(domain: :domain, realm: :realm, latest: :latest, settings: :settings)

      attr_reader :domain, :realm, :latest, :settings

      def initialize(domain:, realm: nil, latest: nil, settings: {})
        @domain   = domain.to_s
        @realm    = realm&.to_s
        @latest   = latest&.to_s
        @settings = settings
      end


    end
  end
end
