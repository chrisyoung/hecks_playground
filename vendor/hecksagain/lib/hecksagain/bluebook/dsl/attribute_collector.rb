module Hecksagain
  module Bluebook
    module DSL
      module AttributeCollector
        ListOf = Struct.new(:type)
        OneOf  = Struct.new(:values)

        # Vendored addition, not (yet) upstream hecksagain (migration plan
        # task 4): shared by BOTH `AttributeCollector#attribute` (below) AND
        # `AggregateBuilder#attribute`'s own override, which needs the
        # CORRECTED name/type BEFORE its own bare-primitive-wrapper decision
        # runs, not after -- calling `super` too late let `attribute App`
        # (no `as:`) reach AggregateBuilder's primitive check as `name: :App,
        # type: String` (the un-fixed shape), which then synthesised a
        # wrapper VO NAMED "App", colliding with a real hand-written one. A
        # module_function here is the one place both layers call, instead of
        # the correction living only where the second caller couldn't reach
        # it in time. See `attribute`'s own comment for why the check itself
        # (`/\A[A-Z]/`) is right.
        def self.resolve_inverted(name, type, as)
          return [name, type] unless name.is_a?(Symbol) && name.to_s.match?(/\A[A-Z]/)

          [as || Naming.snake(name).to_sym, name]
        end

        # Vendored addition, not (yet) upstream hecksagain (migration plan
        # task 8): `attribute :flag, Boolean` -- 55 occurrences across 19
        # hecks_nursury files. `Boolean` is not a real Ruby constant, so it
        # resolves through ConstShim's bare identity resolver to the Symbol
        # `:Boolean`, same mechanism every ordinary type constant already
        # goes through -- but `PRIMITIVE_CLASSES` (AggregateBuilder) only
        # recognises actual Ruby classes (`TrueClass`/`FalseClass` among
        # them), never a Symbol, so `:Boolean` fell straight through to
        # becoming a literal type NAME string `"Boolean"`, which no
        # ValueObject anywhere is ever declared under -- "no ValueObject
        # with ... name Cartography:Datum:Boolean". Shared here (not only
        # in AggregateBuilder#attribute) because `include AttributeCollector`
        # reaches value_object/entity/command/query attributes too, and a
        # bare `Boolean` inside any of THOSE would hit the identical dead
        # end at type-resolution time, just later and less obviously.
        # Mapped to `FalseClass` -- arbitrary between the two boolean
        # classes, but `FalseClass`/`TrueClass` are already treated as
        # interchangeable everywhere else a boolean primitive is checked
        # (IR::Attribute::PRIMITIVES lists both), so either name reads a
        # true/false field correctly. TODO upstream via bin/evolve
        # (migration plan task 7): a first-class `Boolean` alias, not a
        # Symbol-sniffing workaround.
        def self.normalize_boolean_alias(type)
          return FalseClass if type == :Boolean
          return ListOf.new(FalseClass) if type.is_a?(ListOf) && type.type == :Boolean

          type
        end

        def attributes = @attributes ||= []

        # Value objects synthesised from inline closed sets, collected here and
        # installed by whoever owns value objects (the aggregate).
        def closed_sets = @closed_sets ||= []

        # `admits:` names a closed set that is ALREADY DECLARED elsewhere —
        #
        #   attribute :op, String, admits: "Vocabulary::QueryComparator"
        #
        # which is the difference between it and `one_of`: `one_of` SYNTHESISES
        # a fresh value object named for the attribute, so it can only ever name
        # something new. `admits` points at a set the language already holds, so
        # the same set can be named from many places without being written twice.
        #
        # QUALIFIED, because a closed set is a value object INSIDE an aggregate
        # and `reference_to` reaches heads only — so this is text, checked where
        # it is read rather than by reference resolution.
        #
        # AND WRITTEN AS TEXT, not as the constant path `Vocabulary::QueryComparator`
        # it reads like. The constant spelling was tried — `ConstShim` returning
        # a Module so Ruby's `::` reaches a second `const_missing` — and it
        # cannot hold: `Facade::Surface` installs every aggregate name as a
        # TOP-LEVEL constant, so the moment any facade exists, `Vocabulary`
        # resolves to that module and the shim is never asked. A spelling that
        # works only until a facade is built is worse than a quoted one.
        # Vendored addition, not (yet) upstream hecksagain: 147+ lines in
        # hecks_conception's storehouse/bluebook/ (framework kernel) write
        # the INVERTED call shape `attribute TypeConstant, as: :field_name`
        # (or bare `attribute TypeConstant` with no `as:` at all, name
        # inferred from the type) instead of `attribute :field_name,
        # TypeConstant` -- type leads, name comes from `as:` or is derived.
        #
        # CORRECTED detection, not the original (migration plan task 4): the
        # original checked "the first positional arg is NOT a Symbol" -- but
        # `App` (a bare constant reference) resolves through ConstShim's
        # bluebook-scope resolver (`->(const) { const }`, bluebook_builder.rb)
        # to the Symbol `:App`, ALWAYS, the same way every ordinary
        # `attribute :field, TypeConstant` call's OWN type argument already
        # does. So "not a Symbol" can never be true for a bare constant, and
        # the original check silently never fired: `attribute App`
        # (command_bus.bluebook, framework kernel) parsed as `name: :App,
        # type: String` (default) -- a bogus String field literally NAMED
        # "App" -- which then hit the aggregate's bare-primitive auto-
        # synthesis (Part 3a) and tried to mint a wrapper value object ALSO
        # named "App", colliding with the real, hand-written `value_object
        # "App" do ... end` a few lines above it ("Declare creates a
        # ValueObject that already exists").
        #
        # The real, reliable signal: a bare-constant-resolved Symbol is
        # ALWAYS PascalCase (Ruby constants must start uppercase) ; a
        # literal field-name Symbol (`:app`, `:field_name`) is always this
        # corpus's own snake_case convention. `/\A[A-Z]/` tells them apart
        # regardless of whether `as:` was given.
        # `required:` -- vendored alias, not (yet) upstream hecksagain
        # (migration plan task 4): `attribute :project, Project, required:
        # true` (aggregates/plan/bluebook/plan.bluebook, 3 occurrences) --
        # semantically the exact inverse of `optional:`, different word.
        # TODO upstream via bin/evolve (migration plan task 7): decide
        # which spelling becomes canonical.
        # `logged:` -- vendored addition, not (yet) upstream hecksagain
        # (migration plan task 4): see IR::Attribute's own comment.
        # `enum:` -- vendored alias, not (yet) upstream hecksagain
        # (migration plan task 8): `attribute :value, String, enum:
        # ["permit", "forbid"]` (pizzeria/domain/authorization.bluebook) --
        # a Rails-style kwarg spelling of the SAME closed-set concept
        # `one_of(*values)` already sugars in the type position ; the base
        # type ("String") is a placeholder the closed set replaces, same
        # as `one_of`'s own behavior. TODO upstream via bin/evolve
        # (migration plan task 7): decide the one canonical spelling.
        def attribute(name, type = String, as: nil, default: nil, optional: false, required: nil,
                      pattern: nil, admits: nil, logged: true, enum: nil)
          optional = !required unless required.nil?
          type = OneOf.new(enum) if enum
          name, type = AttributeCollector.resolve_inverted(name, type, as)
          type = AttributeCollector.normalize_boolean_alias(type)
          # moved to the language: FieldName invariant, on Root.Attribute

          refuse_unshared_pattern(name, pattern) if pattern

          type = synthesise_closed_set(name, type) if type.is_a?(OneOf)
          list = type.is_a?(ListOf)
          attributes << IR::Attribute.new(
            name:     name,
            type:     list ? type.type : type,
            list:     list,
            default:  default,
            optional: optional,
            pattern:  pattern,
            admits:   admits,
            logged:   logged
          )
        end

        def list_of(type) = ListOf.new(type)

        # A closed set declared INLINE on the attribute:
        #
        #   attribute :status, one_of("open", "shut")
        #
        # Desugars to a value object named for the attribute, so it goes through
        # exactly the machinery a hand-written one_of does — and so the
        # attribute's type is still a DECLARED value object, which is now a
        # structural rule rather than a predicate.
        #
        # An earlier reading of this spelling parsed it and threw the values
        # away: the attribute became a plain String and the closed set meant
        # nothing, in a construct that looked supported. The desugaring is
        # pinned now — the same bluebook must always yield the same IR.
        def one_of(*values) = OneOf.new(values)

        private

        # A pattern is refused AT DECLARATION, not when a value first meets it :
        # a regex whose meaning depends on which engine reads it is a defect in
        # the bluebook, and a bluebook that loads is one whose patterns carry
        # one meaning. PatternSubset says which those are, and why each is refused.
        def refuse_unshared_pattern(name, pattern)
          rejection = PatternSubset.validate(pattern)
          return unless rejection

          raise Malformed,
                "#{name}'s pattern #{pattern.inspect} uses a #{rejection.construct} — " \
                "#{rejection.reason}"
        end

        def synthesise_closed_set(name, one_of)
          type = Naming.pascal(name)
          closed_sets << IR::ValueObject.declare(
            name:       type,
            attributes: [IR::Attribute.new(name: :value, type: "String")],
            members:    one_of.values.map { |value| { value: value.to_s } },
            closed_set: true
          )
          type
        end
      end
    end
  end
end
