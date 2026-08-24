require_relative "behaviour/process_manager"
require_relative "../ir"

module Hecks
  module Bluebook
    # Vendored addition, not (yet) upstream hecks (migration plan
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

    DispatchSpec = Struct.new(:command_name, :with_spec, :for_each, keyword_init: true) do
      # A Struct already answers to_h; including the mixin puts the
      # DECLARED emission ahead of Struct's own in the ancestry, which
      # is what makes the shape data rather than a method body.
      include Hecks::IR

      emits_ir(
        command_name: -> { command_name.to_s },
        with_spec:    -> { with_spec.map { |key, value| [key.to_s, Bluebook.render_value(value)] } },
        # `for_each`, vendored addition not (yet) upstream hecks
        # (migration plan task 4, i225): the FQN of a query ("Signal.cold")
        # to enumerate — nil for an ordinary single dispatch. See
        # Runtime::SagaInterpreter#deliver_saga_dispatch's own comment for
        # the runtime side.
        for_each:     :for_each
      )
    end

    ProcessManagerHandler = Struct.new(:event_type, :from_state, :to_state,
                                       :dispatches, :remembers, :guards, :guard_count, keyword_init: true) do
      include Hecks::IR

      # `guard_count` — vendored addition, not (yet) upstream hecks
      # (migration plan task 4, i768 follow-up): overrides the Struct's
      # own generated reader so EVERY caller (`to_h`'s own emits_ir lambda
      # below, AND `MetaValidator::Judge#declare_node`'s generic
      # `node.public_send(reader)` walk, which reads the raw attribute
      # directly, never through `to_h`) sees the same computed value —
      # the everyday DSL::ProcessManagerBuilder path never sets the raw
      # Struct member (guards stays real Procs, counted here on demand) ;
      # only Assembly's meta-domain reconstruction sets it explicitly,
      # since raw Procs can't round-trip through the Judge. Without this
      # override the Judge declared "0" (from `to_h`) but read back "nil"
      # (from the raw, never-set Struct member) for every ordinary,
      # guard-less handler — `round_trip_spec` caught it directly.
      def guard_count
        self[:guard_count] || (guards || []).size
      end

      emits_ir(
        event_type:  -> { event_type.to_s },
        from_state:  -> { from_state.to_s },
        to_state:    -> { to_state.to_s },
        dispatches:  many(:dispatches),
        # `remembers`, vendored addition not (yet) upstream hecks
        # (migration plan task 4): named values this handler writes into
        # the saga instance's OWN carried memory, for a LATER handler on
        # the same instance to read back via `from_pm`. See
        # DSL::ProcessManagerBuilder::HandlerBuilder#remember and
        # Runtime::SagaInterpreter#advance_saga's own comment for the
        # read side.
        remembers:   -> { (remembers || []).map { |k, v| [k.to_s, Bluebook.render_value(v)] } },
        # `guards`, vendored addition not (yet) upstream hecks
        # (migration plan task 4): raw Procs, not rendered — same reason
        # a command's `given`/`ensures` predicates aren't either (Proc
        # isn't JSON-shaped); only their PRESENCE is exported, as a
        # count, so the IR still says a guard exists without claiming to
        # describe it. See DSL::ProcessManagerBuilder::HandlerBuilder#given.
        #
        # `guard_count` is a real, SEPARATE seventh member (i768 follow-up
        # fix) — the everyday DSL::ProcessManagerBuilder path never sets
        # it (guards stays real Procs, counted here on demand) ; only
        # Assembly's meta-domain reconstruction sets it explicitly, since
        # raw Procs can't round-trip through the Judge. Preferring the
        # explicit value when Assembly set one, falling back to counting
        # the real guards otherwise, means both paths answer the same
        # question the same way. See the overridden `#guard_count` reader
        # above — this lambda is now just a plain `Symbol` field read
        # like every other, kept as a `Proc` only so the doc comment has
        # somewhere to live beside its siblings.
        guard_count: -> { guard_count }
      )
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
      # The trigger of a compensating leg. Not an event name — no aggregate
      # announces that a leg the procedure dispatched was declined — so it lives
      # here beside the thing it triggers rather than in the runtime that
      # notices it. Declared in the language's Trigger vocabulary, which
      # spec/vocabulary_conformance_spec holds to this constant.
      REFUSED = Hecks::Vocabulary.fetch("Trigger").first

      include Hecks::IR
      include Behaviour::ProcessManager

      emits_ir(
        name:          :name,
        correlates_by: -> { correlates_by.to_s },
        starts_on:     :starts_on,
        ends_on:       :ends_on,
        states:        :states,
        handlers:      many(:handlers)
      )

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



    end
  end
end
