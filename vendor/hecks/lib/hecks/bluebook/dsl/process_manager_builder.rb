module Hecks
  module Bluebook
    module DSL
      class ProcessManagerBuilder
        class InvalidProcessManager < StandardError; end

        def initialize(name)
          @name     = name
          @handlers = []
        end

        def correlates_by(field) = @correlates_by = field.to_sym
        def starts_on(event)     = @starts_on = event.to_s
        def ends_on(event)       = @ends_on = event.to_s

        # ONE STATE-MACHINE VOCABULARY (S7, ADR 0025 — "events and
        # reactions"): the SAME word `Lifecycle#transition` already
        # carries, one level over — `transition "AccountDebited" =>
        # "awaiting_credit", from: "requested" do ... end` replaces `on
        # "AccountDebited", transition: { "requested" => "awaiting_
        # credit" } do ... end`. Same bare rocket-pair argument shape
        # (not a NAMED `transition:` kwarg wrapping a second Hash), same
        # `from:` — including the array form Lifecycle's own commands
        # could already take and a process manager's own events could
        # not — and the states a procedure runs on are DERIVED from the
        # transitions that name them, the same way `Behaviour::Lifecycle
        # #states` already derives an aggregate's ; `state "x"` lines
        # duplicated exactly what the transition list already said,
        # and could drift from it (`validate!`'s own "undeclared state"
        # check existed only because they could).
        #
        # `starts_on`/`ends_on` are NOT unified into this — verified
        # against the real corpus rather than assumed: Settlement's own
        # `ends_on "TransferSettled"` names an event NONE of its own
        # transitions ever handle (`Transfer.Settle`'s own emission, a
        # full step downstream of the transition that dispatches it),
        # so "the terminal state's own event" is not a fact the
        # transition graph carries — deriving it would either be wrong
        # for this exact corpus member or need a second new word to
        # cover the case, which is not less vocabulary than keeping the
        # one that already says it correctly.
        #
        # EXPANDS IMMEDIATELY, unlike `Lifecycle#transition` (which
        # defers to `Behaviour::Lifecycle#expand`, called at emission
        # time) — `ProcessManager`'s own IR constructor takes `states:`/
        # `handlers:` exactly as it always has, so the runtime
        # (`Behaviour::ProcessManager`, `SagaInterpreter`, saga
        # persistence/rehydration) needs no change at all: what changed
        # is how the DECLARATION reaches that same shape, not the shape
        # a real run ever sees or persists.
        def transition(mapping, &block)
          mapping = mapping.dup
          from    = mapping.delete(:from)

          # ALWAYS REQUIRED, unlike `Lifecycle#transition`'s own `from:`
          # — an aggregate's unconstrained transition is admitted from
          # ANY current state (`Behaviour::Lifecycle#applies_from?`
          # returns true for a nil `from`), a reading `SagaInterpreter#
          # advance_saga`'s own admission check does not share: it tests
          # `instance[:state] == handler.from_state` by plain equality,
          # nothing softer. Leaving that check unchanged (this slice's
          # own scope decision — see the class-level comment on why the
          # runtime stays untouched) means an unconstrained PM
          # transition would build cleanly and then match no instance
          # ever, silently — refused here instead, at the one point that
          # can still see the mistake.
          if from.nil?
            raise InvalidProcessManager,
                  "#{@name}'s transition #{mapping.inspect} names no from: — a process manager's own " \
                  "admission checks a saga instance's CURRENT state exactly, so a transition with no " \
                  "from: would match no instance ever, silently"
          end

          handler = HandlerBuilder.new
          handler.instance_eval(&block) if block

          mapping.each do |event_type, target|
            state_transition = StateTransition.new(target: target, from: from)
            expand(event_type.to_s, state_transition, handler.dispatches,
                   handler.remembers, handler.guards).each { |row| @handlers << row }
          end
        end

        def build
          validate!

          ProcessManager.new(
            name:          @name,
            correlates_by: @correlates_by,
            starts_on:     @starts_on,
            ends_on:       @ends_on,
            states:        derived_states,
            handlers:      @handlers
          )
        end

        def self.build(name, &block)
          builder = new(name)
          builder.instance_eval(&block) if block
          builder.build
        end

        private

        # ONE DECLARED TRANSITION IS SEVERAL ROWS when `from` names more
        # than one source state — `Behaviour::Lifecycle#expand`'s own
        # comment, the identical fan-out, one level over: a
        # `ProcessManagerHandler` only ever carries a single `from_state`,
        # so a `from: [...]` transition mints one row per source, each
        # carrying the SAME dispatches.
        def expand(event_type, transition, dispatches, remembers = [], guards = [])
          sources = transition.from.nil? ? [nil] : Array(transition.from)

          sources.map do |source|
            ProcessManagerHandler.new(
              event_type: event_type,
              from_state: source.to_s,
              to_state:   transition.target,
              dispatches: dispatches,
              remembers:  remembers,
              guards:     guards
            )
          end
        end

        # DERIVED, not declared (S7) — every state this procedure ever
        # runs on is already named by some transition's own `from_state`
        # or `to_state`; a state nothing transitions into or out of is
        # not a state this procedure has, the same reading
        # `Behaviour::Lifecycle#states` already gives an aggregate's own
        # field. FIRST-SEEN ORDER, walking declaration order — `begin_
        # saga`'s own `pm.states.first` is what a fresh instance starts
        # in, so the order has to survive the derivation, not just the
        # membership.
        def derived_states
          @handlers.flat_map { |h| [h.from_state, h.to_state] }.reject(&:empty?).uniq
        end

        def validate!
          raise InvalidProcessManager, "#{@name} declares no correlates_by — " \
            "nothing would tie its events to one instance" unless @correlates_by

          # THE FIELD, NAMED — never the value object that carries it. A bare
          # `correlates_by :end_to_end` reads whatever the payload holds under
          # that key AS the correlation key, and what a non-scalar key even
          # is stays open (the object itself? its serialised text?).
          # Requiring the dotted spelling —
          # `:"end_to_end.value"` — makes every correlates_by name a scalar
          # by construction, the same discipline `identified_by` already
          # holds a head to. This is a syntactic check, not a type check: it
          # does not know or care whether the field IS a value object, only
          # that the declaration cannot leave that question open.
          raise InvalidProcessManager, "#{@name} correlates_by #{@correlates_by.inspect}, which names a whole " \
            "field rather than one of its scalars — say which one, e.g. " \
            "#{@correlates_by}.value" unless @correlates_by.to_s.include?(".")

          raise InvalidProcessManager, "#{@name} declares no starts_on — " \
            "nothing would ever begin it" if @starts_on.to_s.empty?

          raise InvalidProcessManager, "#{@name} declares no transitions — " \
            "it would start and then ignore every event" if @handlers.empty?
        end

        class HandlerBuilder
          attr_reader :dispatches, :remembers, :guards

          def initialize
            @dispatches = []
            @remembers  = []
            @guards     = []
          end

          # THE COMMAND ITSELF (ADR 0025, "events and reactions" — command
          # references become first-class), same shape and same reasons
          # as `PolicyBuilder#trigger`'s own header — bare constant live,
          # quoted text only under shadow-parsing (S0a's bridge; frozen
          # era text still writes `dispatch "Banking::Account.Debit"`).
          #
          # `for_each: { from: "Aggregate.query" }` -- vendored addition,
          # not (yet) upstream hecks (migration plan task 4, i225):
          # enumerate the named query and fire this dispatch once per
          # returned row, each field sourced via `from_iter(:field)`
          # against THAT row rather than the triggering event. See
          # Runtime::SagaInterpreter#deliver_saga_dispatch.
          def dispatch(command_ref, with: nil, for_each: nil)
            if command_ref.is_a?(::String) && !MetaValidator.shadow_parsing?
              raise InvalidProcessManager,
                    "dispatch #{command_ref.inspect} is quoted text — give the bare command constant " \
                    "instead, e.g. dispatch Account::Debit"
            end

            @dispatches << DispatchSpec.new(
              command_name: Naming.command_ref(command_ref),
              with_spec:    (with || {}).to_a,
              for_each:     for_each && for_each.fetch(:from)
            )
          end

          # `given { |ctx| ... }` -- vendored addition, not (yet) upstream
          # hecks (migration plan task 4): a handler-level
          # precondition on its DISPATCHES, not on the transition itself
          # -- the transition + any `remember`s always happen when the
          # event fires in the right from_state; `given` only decides
          # whether this handler's `dispatch`es actually fire. `ctx` is a
          # merged Hash of the triggering event's payload + the saga
          # instance's own remembered fields — see Runtime::
          # SagaInterpreter#advance_saga's own comment for how it's built
          # and evaluated.
          def given(&predicate)
            @guards << predicate
          end

          # `remember key: from_event(...)` -- vendored addition, not
          # (yet) upstream hecks (migration plan task 4). Writes
          # into the saga instance's OWN carried memory (Runtime::
          # SagaInterpreter's `instance[:memory]`) under `key`, for a
          # LATER handler on the same instance to read back via
          # `from_pm(:key)`. `instance[:memory]` was write-once before
          # this, so `from_pm` could only ever see the starting event,
          # never anything a mid-saga handler decided.
          def remember(**fields)
            @remembers.concat(fields.to_a)
          end

          # `set :field, from_event(:field)` -- vendored addition, not
          # (yet) upstream hecks (migration plan task 8, bin-buddy's
          # SubscriptionLifecycle PM): the POSITIONAL-argument sibling of
          # `remember key: from_event(...)` -- same accumulator, same
          # saga-memory write, just written field-then-value instead of
          # as a kwarg. Kept as a real alias, not a rename -- `remember`
          # stays the kwarg form other corpora already use.
          def set(field, value)
            @remembers << [field.to_sym, value]
          end

          # Vendored addition, not (yet) upstream hecks: `with: {
          # key: from_event(:field, default: "x") }` -- explicit sugar
          # for "read :field from the triggering event's payload,
          # falling back to default if absent". Returns the bare Symbol,
          # relying on with_spec's EXISTING Symbol-means-argument-
          # reference-resolved-at-dispatch-time mechanism to source it
          # from the event payload the same way any other argument
          # reference already does -- the `default:` fallback is NOT
          # threaded through (a real, documented gap: an absent field
          # currently resolves to nil, not the declared default).
          def from_event(field, default: nil) = field

          # Same shape as from_event, sourcing from an iteration/loop
          # variable instead of the event payload -- see from_event's
          # own comment for the same documented default: gap.
          def from_iter(field, default: nil) = field

          # Same shape again, sourcing from the PROCESS MANAGER'S OWN
          # persisted state instead of the triggering event or an
          # iteration variable. Same documented default:-not-threaded
          # gap as its two siblings.
          def from_pm(field, default: nil) = field

          # `template("fmt %s", from_pm(:x, default: "y"))` -- vendored
          # addition, not (yet) upstream hecks (migration plan task
          # 4). See Bluebook::TemplateSpec's own comment for why.
          # Inherits the SAME documented default:-not-threaded gap as
          # from_event/from_iter/from_pm -- a `default:` on an ARGUMENT
          # passed into a template still resolves to nil, not the
          # fallback, if the field is absent; this call adds no new gap,
          # just composes with an existing one.
          def template(format, *args) = TemplateSpec.new(format: format, args: args)
        end
      end
    end
  end
end
