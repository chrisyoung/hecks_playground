module Hecksagain
  module Bluebook
    module DSL
      class ValueObjectBuilder
        include AttributeCollector

        def initialize(name)
          @name       = name
          @invariants = []
          @members    = []
        end

        # Vendored no-op stub, not (yet) upstream hecksagain -- see
        # CommandBuilder/PolicyBuilder's identical stubs.
        def description(value) = nil

        # moved to the language: a closed set admits a member, on Shape.Close.
        #
        # This was called unportable because an empty one_of and no one_of are
        # both `members: []` in the IR. That was a MODELLING choice, not a law —
        # an empty attribute NAME survives into the IR and is judged there. So
        # the IR now records that a closed set was declared, and the language
        # judges it like everything else.
        # Vendored disambiguation, not (yet) upstream hecksagain: the bare
        # word `one_of` names TWO different grammar forms that collide
        # inside a value_object body specifically -- the closed-set BODY
        # form (`one_of do member value: "x" end`, this class's own
        # method) and AttributeCollector's TYPE-position sugar
        # (`attribute :x, one_of("a", "b")`), needed when a value_object's
        # OWN attribute wants an inline closed set. Since ValueObjectBuilder
        # overrides AttributeCollector's `one_of`, the sugar form silently
        # resolved to this 0-arg method and raised ArgumentError
        # (hecks_conception's ViolationKind value_object hit this). Args-only
        # (no block) delegates to `super` -- AttributeCollector's original;
        # block-only (no args) keeps this class's own closed-set-body
        # behavior. TODO upstream via hecksagain's own bin/evolve
        # word-admission process (migration plan task 7).
        def one_of(*values, &block)
          return super(*values) if values.any? && !block

          @closed_set = true
          instance_eval(&block) if block
        end

        def member(**fields)
          raise Malformed, "#{@name} declared an empty member" if fields.empty?

          @members << fields
        end

        def invariant(description = "an invariant holds", &predicate)
          canonical = Ports::Extraction.canonical(predicate)

          # moved to the language: given "a rule says what it means", on Shape.Assert

          if canonical.to_s.empty?
            raise Malformed,
                  "#{@name}'s invariant #{description.inspect} did not survive " \
                  "extraction — it would be a rule the IR cannot carry"
          end

          @invariants << IR::Invariant.new(
            description: description,
            canonical:   canonical,
            predicate:   predicate
          )
        end
        # Vendored alias, not (yet) upstream hecksagain (migration plan
        # task 8): `rule "description" do ... end` -- 21 files across the
        # other-20-projects wave (bin-buddy/opt-website/vindiction/
        # mietteai/...) write this instead of `invariant`. Not a random
        # guess : this method's OWN existing comment already names "rule"
        # as the intended word ("moved to the language: given 'a rule
        # says what it means', on Shape.Assert") -- the alias just
        # exposes the vocabulary hecksagain's own source already settled
        # on. TODO upstream via bin/evolve (migration plan task 7):
        # decide which spelling becomes canonical.
        alias_method :rule, :invariant

        def build
          # `attribute :size, one_of("small", "large")` INSIDE a value_object
          # body reaches AttributeCollector#synthesise_closed_set (via the
          # one_of/#one_of disambiguation this class's own `one_of` method
          # already fixed — no more ArgumentError on the wrong arity) and
          # pushes a real closed-set ValueObject onto `closed_sets` — but
          # `IR::ValueObject.declare` has no member to hold a NESTED value
          # object at all (unlike IR::Aggregate, which merges its own
          # `closed_sets` into `value_objects`). Silently dropping it here
          # left `:size` typed as a reference to a shape that was never
          # registered anywhere — no crash, no refusal, and `aggregate.
          # value_object("Size")` simply returned nil the first time anyone
          # actually looked. Found live via docs/guides/aggregates-and-
          # value-objects.md's own doctest, which the syntax-level
          # disambiguation fix turned from "raises ArgumentError" into
          # "loads clean and is silently broken" — worse, not fixed.
          # Refused here instead, honestly, until value objects can really
          # nest one (a structural change well beyond this fix's scope).
          if closed_sets.any?
            raise Malformed,
                  "#{@name}'s attribute #{closed_sets.first.hecks_name.inspect} names an inline " \
                  "one_of, which a value object cannot hold — value objects do not nest. " \
                  "Declare the closed set as its own value_object instead."
          end

          IR::ValueObject.declare(
            name: @name, attributes: attributes,
            invariants: @invariants, members: @members,
            closed_set: @closed_set || !@members.empty?
          )
        end

        def self.build(name, &block)
          builder = new(name)
          builder.instance_eval(&block) if block
          builder.build
        end
      end
    end
  end
end
