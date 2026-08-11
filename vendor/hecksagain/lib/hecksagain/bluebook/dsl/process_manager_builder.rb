module Hecksagain
  module Bluebook
    module DSL
      class ProcessManagerBuilder
        class InvalidProcessManager < StandardError; end

        def initialize(name)
          @name     = name
          @states   = []
          @handlers = []
        end

        def correlates_by(field) = @correlates_by = field.to_sym
        def starts_on(event)     = @starts_on = event.to_s
        def ends_on(event)       = @ends_on = event.to_s
        def state(name)          = @states << name.to_s

        # `description` -- vendored addition, not (yet) upstream hecksagain
        # (migration plan task 4): prose-only, matches PolicyBuilder's own
        # accept-and-discard stub. A process manager's narrative belongs to
        # the corpus author, not the IR.
        def description(value) = nil

        def on(event_type, transition:, &block)
          from, to = single_transition(event_type, transition)
          handler  = HandlerBuilder.new
          handler.instance_eval(&block) if block

          @handlers << IR::ProcessManagerHandler.new(
            event_type: event_type.to_s,
            from_state: from,
            to_state:   to,
            dispatches: handler.dispatches,
            remembers:  handler.remembers,
            guards:     handler.guards
          )
        end

        def build
          validate!

          IR::ProcessManager.new(
            name:          @name,
            correlates_by: @correlates_by,
            starts_on:     @starts_on,
            ends_on:       @ends_on,
            states:        @states,
            handlers:      @handlers
          )
        end

        def self.build(name, &block)
          builder = new(name)
          builder.instance_eval(&block) if block
          builder.build
        end

        private

        def single_transition(event_type, transition)
          unless transition.is_a?(Hash) && transition.size == 1
            raise InvalidProcessManager,
                  "#{@name}.on(#{event_type.inspect}) needs exactly one " \
                  "from => to transition, got #{transition.inspect}"
          end

          from, to = transition.first
          [from.to_s, to.to_s]
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

          raise InvalidProcessManager, "#{@name} declares no states" if @states.empty?

          raise InvalidProcessManager, "#{@name} declares no handlers — " \
            "it would start and then ignore every event" if @handlers.empty?

          undeclared = @handlers.flat_map { |h| [h.from_state, h.to_state] }
                                .uniq
                                .reject { |s| @states.include?(s) }
          return if undeclared.empty?

          raise InvalidProcessManager,
                "#{@name} transitions through #{undeclared.map(&:inspect).join(', ')} " \
                "which #{undeclared.size == 1 ? 'is' : 'are'} never declared as a state"
        end

        class HandlerBuilder
          attr_reader :dispatches, :remembers, :guards

          def initialize
            @dispatches = []
            @remembers  = []
            @guards     = []
          end

          # `given { |ctx| ... }` -- vendored addition, not (yet) upstream
          # hecksagain (migration plan task 4): a handler-level
          # precondition on its DISPATCHES, not on the transition itself
          # -- the transition + any `remember`s always happen when the
          # event fires in the right from_state; `given` only decides
          # whether this handler's `dispatch`es actually fire. Real need,
          # not speculative: miette's Lucidity PM enters `rejoined` on
          # `LucidRemEntered` regardless of whether Mind is present (the
          # bridge's Body-half alone is enough to transition), but only
          # dispatches `LucidDream.BecomeLucid` when Mind actually is —
          # the OLD imperative Proc form did `next({commands: []}) unless
          # mind_present` after the transition had already been decided
          # by the enclosing framework, same split. `ctx` is a merged
          # Hash of the triggering event's payload + the saga instance's
          # own remembered fields — see Runtime::SagaInterpreter#
          # advance_saga's own comment for how it's built and evaluated.
          def given(&predicate)
            @guards << predicate
          end

          # `remember key: from_event(...)` -- vendored addition, not
          # (yet) upstream hecksagain (migration plan task 4). Writes
          # into the saga instance's OWN carried memory (Runtime::
          # SagaInterpreter's `instance[:memory]`) under `key`, for a
          # LATER handler on the same instance to read back via
          # `from_pm(:key)`. See IR::ProcessManagerHandler's own comment
          # on why this exists -- `instance[:memory]` was write-once
          # before this, so `from_pm` could only ever see the starting
          # event, never anything a mid-saga handler decided.
          def remember(**fields)
            @remembers.concat(fields.to_a)
          end

          # `set :field, from_event(:field)` -- vendored addition, not
          # (yet) upstream hecksagain (migration plan task 8, bin-buddy's
          # SubscriptionLifecycle PM): the POSITIONAL-argument sibling of
          # `remember key: from_event(...)` -- same accumulator, same
          # saga-memory write, just written field-then-value instead of
          # as a kwarg (bin-buddy's own corpus convention, found live
          # rather than invented). Kept as a real alias, not a rename --
          # `remember` stays the kwarg form other corpora already use.
          def set(field, value)
            @remembers << [field.to_sym, value]
          end

          # `for_each: { from: "Aggregate.query" }` -- vendored addition,
          # not (yet) upstream hecksagain (migration plan task 4, i225):
          # enumerate the named query and fire this dispatch once per
          # returned row, each field sourced via `from_iter(:field)`
          # against THAT row rather than the triggering event. See
          # Runtime::SagaInterpreter#deliver_saga_dispatch.
          def dispatch(command_name, with: nil, for_each: nil)
            @dispatches << IR::DispatchSpec.new(
              command_name: command_name,
              with_spec:    (with || {}).to_a,
              for_each:     for_each && for_each.fetch(:from)
            )
          end

          # Vendored addition, not (yet) upstream hecksagain: `with: {
          # key: from_event(:field, default: "x") }` (miette's
          # body/pulse_organs/bluebook and body/sleep/consolidation/
          # bluebook) -- explicit sugar for "read :field from the
          # triggering event's payload, falling back to default if
          # absent". Returns the bare Symbol, relying on with_spec's
          # EXISTING Symbol-means-argument-reference-resolved-at-
          # dispatch-time mechanism to source it from the event payload
          # the same way any other argument reference already does --
          # the `default:` fallback is NOT threaded through (a real,
          # documented gap: an absent field currently resolves to nil,
          # not the declared default). TODO upstream via bin/evolve
          # (migration plan task 7).
          def from_event(field, default: nil) = field

          # Same shape as from_event, sourcing from an iteration/loop
          # variable instead of the event payload -- see from_event's
          # own comment for the same documented default: gap.
          def from_iter(field, default: nil) = field

          # `template("fmt %s", from_pm(:x, default: "y"))` -- vendored
          # addition, not (yet) upstream hecksagain (migration plan task
          # 4). See IR::TemplateSpec's own comment for why. Inherits the
          # SAME documented default:-not-threaded gap as from_event/
          # from_iter/from_pm -- a `default:` on an ARGUMENT passed into
          # a template still resolves to nil, not the fallback, if the
          # field is absent; this call adds no new gap, just composes
          # with an existing one.
          def template(format, *args) = IR::TemplateSpec.new(format: format, args: args)

          # Same shape again, sourcing from the PROCESS MANAGER'S OWN
          # persisted state instead of the triggering event or an
          # iteration variable -- the third of the same family (miette's
          # mind/bluebook: `from_pm(:carrying, default: "—")`, reading a
          # field the PM itself carries across ticks, not something the
          # current event/iteration handed it). Same documented
          # default:-not-threaded gap as its two siblings.
          def from_pm(field, default: nil) = field
        end
      end
    end
  end
end
