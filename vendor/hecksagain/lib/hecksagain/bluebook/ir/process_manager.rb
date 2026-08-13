module Hecksagain
  module Bluebook
    module IR
      # Vendored addition, not (yet) upstream hecksagain (migration plan
      # task 4): `template("fmt %s", from_pm(:x, default: "y"))` inside a
      # `dispatch ..., with: { field: template(...) }` -- composing a
      # literal string with a resolved field, which no `with:` value
      # spelling could do before this (a bare Symbol IS a whole resolved
      # value; there was no way to embed one inside surrounding text).
      # Found live in miette's mind.bluebook ("I'd like to go deeper into
      # #{target} with this" -- the OLD imperative Proc form's own string
      # interpolation, which the file's own comment already named as
      # needing "a template: form" before it could convert). `args` holds
      # whatever `dispatch_args`/`resolve_with_value` already know how to
      # resolve -- bare Symbols (from_event/from_pm/from_iter) or
      # literals -- resolved the SAME way an ordinary `with:` value is,
      # just substituted into `format` via `Kernel#format` rather than
      # assigned directly. See Runtime::SagaInterpreter#resolve_value's
      # own comment for the read side.
      TemplateSpec = Struct.new(:format, :args, keyword_init: true)

      # `for_each`, vendored addition not (yet) upstream hecksagain
      # (migration plan task 4, i225): the FQN of a query ("Signal.cold")
      # to enumerate — nil for an ordinary single dispatch. See
      # Runtime::SagaInterpreter#deliver_saga_dispatch's own comment for
      # the runtime side.
      DispatchSpec = Struct.new(:command_name, :with_spec, :for_each, keyword_init: true) do
        def to_h
          {
            command_name: command_name.to_s,
            with_spec:    with_spec.map { |key, value| [key.to_s, IR.render_value(value)] },
            for_each:     for_each
          }
        end
      end

      # `remembers`, vendored addition not (yet) upstream hecksagain
      # (migration plan task 4): named values this handler writes into
      # the saga instance's OWN carried memory, for a LATER handler on
      # the same instance to read back via `from_pm`. A real gap this
      # closes: `instance[:memory]` (Runtime::SagaInterpreter) was set
      # ONCE, from the STARTING event's payload, and never mutated again
      # -- so `from_pm` (added earlier this session) could only ever see
      # what the FIRST event carried, never anything a later handler
      # decided along the way (miette's Lucidity PM: `MindSteered` sets
      # the steering target ; the NEXT tick's handler needs to read it
      # back, and neither the starts_on event nor the current tick's own
      # event carries it). `remember key: from_event(...)` is the
      # explicit, named write; see HandlerBuilder#remember and
      # Runtime::SagaInterpreter#advance_saga's own comment for the read
      # side.
      # `guards`, vendored addition not (yet) upstream hecksagain
      # (migration plan task 4): raw Procs, not rendered into `to_h` --
      # same reason a command's `given`/`ensures` predicates aren't
      # either (Proc isn't JSON-shaped); only their PRESENCE is exported,
      # as a count, so the IR still says a guard exists without claiming
      # to describe it. See DSL::ProcessManagerBuilder::HandlerBuilder#given.
      # `guard_count` is a SEVENTH member, not just a computed reader —
      # the self-hosted "Handler" aggregate (reaction.bluebook) declares
      # it as a real attribute (guards themselves, raw Procs, are not
      # storable), so Assembly's reconstruction needs a real keyword to
      # hand the count back through. The everyday DSL::
      # ProcessManagerBuilder path never sets it (only `guards`, real
      # predicates) — `guard_count` stays nil there, and the reader below
      # falls back to counting them, so ordinary dispatch is unchanged.
      ProcessManagerHandler = Struct.new(:event_type, :from_state, :to_state,
                                         :dispatches, :remembers, :guards, :guard_count,
                                         keyword_init: true) do
        # OVERRIDES the plain Struct accessor: an EXPLICIT guard_count (set
        # only by Assembly's reconstruction, which cannot rebuild the raw
        # Procs `guards` normally holds) wins ; otherwise it is computed
        # the way it always was. `self[:guard_count]`, not `guard_count`,
        # reads the underlying member slot directly — calling the accessor
        # here would recurse.
        def guard_count = self[:guard_count] || (guards || []).size

        def to_h
          {
            event_type: event_type.to_s,
            from_state: from_state.to_s,
            to_state:   to_state.to_s,
            dispatches: dispatches.map(&:to_h),
            remembers:  (remembers || []).map { |k, v| [k.to_s, IR.render_value(v)] },
            guard_count: guard_count
          }
        end
      end

      # The compensation half of a procedure, as its own thing.
      #
      # A PROCESS MANAGER coordinates: legs, states, an opinion about who goes
      # next. A SAGA undoes: what makes the world good again when a leg it
      # dispatched is refused. Two concepts, and the industry slurs them into one
      # word — so here they are two objects, and a procedure either has a saga or
      # does not.
      #
      # `undoes` is the ordered list of commands the compensation sends. Today that
      # order is the AUTHOR's, written by hand in one `on :refused` leg, and the
      # runtime does not know which legs actually completed. When compensation
      # moves beside each dispatch — `reverses` on the step it reverses — this is
      # where the completed ones, newest first, will live. The shape is already
      # right for it; only the source of the order changes.
      Saga = Struct.new(:trigger, :from_state, :to_state, :reversals, keyword_init: true) do
        def undoes = reversals.map(&:command_name)

        def to_s = "#{trigger} → #{to_state} (#{undoes.join(', ')})"
      end

      class ProcessManager

        # The BLUEBOOK's name for this construct, asked the same way of a class
        # that has crossed over and of an IR object that has not. Collapses into
        # Construct when this one crosses.
        def hecks_name = @name
        # The trigger of a compensating leg. Not an event name — no aggregate
        # announces that a leg the procedure dispatched was declined — so it lives
        # here beside the thing it triggers rather than in the runtime that
        # notices it. Declared in the language's Trigger vocabulary, which
        # spec/vocabulary_conformance_spec holds to this constant.
        REFUSED = "refused".freeze

        attr_reader :name, :correlates_by, :starts_on, :ends_on, :states, :handlers

        def initialize(name:, correlates_by: nil, starts_on: nil, ends_on: nil,
                       states: [], handlers: [])
          @name          = name.to_s
          @correlates_by = correlates_by
          @starts_on     = starts_on
          @ends_on       = ends_on
          @states        = states
          @handlers      = handlers
        end

        def handler_for(event) = @handlers.find { |h| h.event_type == event.to_s }

        # THE HEAD OF THE DOTTED PATH, and a different question from
        # `correlates_by` itself. `correlates_by` says which SCALAR a fresh
        # event correlates on — the dotted path a value object has to be dug
        # through to reach. This says what a DOWNSTREAM DISPATCH is allowed to
        # call the already-resolved scalar it carries forward (an argument
        # name legal on any command in the domain, and the symbol a `with:`
        # value has to spell to receive the correlation) — a plain identifier
        # a value object was never involved in, so it never needed the dot.
        def correlation_head = @correlates_by.to_s.split(".").first.to_sym

        # A PROCEDURE coordinates: legs, states, who goes next. It is a SAGA when
        # it also knows how to undo itself.
        #
        # The two are different things and the industry slurs them together. A
        # hiring pipeline is a procedure with no saga in it — you cannot
        # un-interview somebody. A choreographed refund is a saga with no
        # procedure — each party knows its own undo and nobody is in charge.
        # Banking's settlement is both.
        #
        # NAMED here and nowhere an author can type it. `saga` is not a word a
        # bank says, so it never appears in a .bluebook — the author declares a
        # compensating leg and the saga follows. Derived, so it cannot drift from
        # the thing it describes, and deliberately NOT in to_h : the IR export
        # spells the SOURCE, and a derived fact is not a fact
        # about the source.
        #
        # nil for a procedure with no answer to a refusal, which is a legitimate
        # thing to be — a hiring pipeline cannot un-interview anybody.
        def saga
          leg = handler_for(REFUSED)
          return nil unless leg

          Saga.new(trigger: REFUSED, from_state: leg.from_state,
                   to_state: leg.to_state, reversals: leg.dispatches)
        end

        def saga? = !saga.nil?

        def to_h
          {
            name:          @name,
            correlates_by: @correlates_by.to_s,
            starts_on:     @starts_on,
            ends_on:       @ends_on,
            states:        @states,
            handlers:      @handlers.map(&:to_h)
          }
        end
      end
    end
  end
end
