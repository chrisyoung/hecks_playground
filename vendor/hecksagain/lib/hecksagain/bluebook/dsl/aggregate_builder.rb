module Hecksagain
  module Bluebook
    module DSL
      class AggregateBuilder
        include AttributeCollector

        def initialize(name)
          @name          = name
          @value_objects = []
          @commands      = []
          @identity_paths = []
          @entities      = []
          @queries       = []
          @policies      = []
          @reference_targets = []
        end

        def description(value)
          # moved to the language: Description invariant, on Root.Declare

          @description = value
        end

        # Vendored addition, not (yet) upstream hecksagain: hecksagain's
        # own named principle is "primitives live in value objects
        # only" (lib/hecksagain/language/bluebook/vocabulary.bluebook) --
        # a bare `attribute :x, String` directly on an aggregate is
        # refused by the self-hosted meta-validator. hecks_conception +
        # miette's own convention does this 260+ times at the aggregate
        # level (value_object-internal bare types were always fine and
        # are untouched here). DECISION, documented not hidden: rather
        # than a corpus-wide rewrite touching every declaration AND
        # every call site that constructs these attributes, this
        # transparently AUTO-SYNTHESISES a single-field wrapper value
        # object per bare-primitive aggregate attribute -- the exact
        # same mechanism `one_of(...)`'s `synthesise_closed_set` already
        # uses for inline closed sets, just triggered by a bare
        # primitive Class instead of a OneOf. Preserves hecksagain's own
        # "primitives only live in VOs" invariant (the VO now just has
        # an author of "the runtime" instead of "the corpus") rather
        # than relaxing it. TODO upstream via bin/evolve (migration plan
        # task 7) -- or supersede with the real corpus-wide VO-wrapping
        # pass if Chris prefers that path instead.
        PRIMITIVE_CLASSES = [String, Integer, Float, TrueClass, FalseClass, Numeric].freeze

        # NAMED, NOT **kwargs — spelled out exactly like AttributeCollector#
        # attribute's own signature (the method this overrides). A `**kwargs`
        # catch-all forwards fine at runtime but ERASES the signature a
        # projected parser reads: spec/syntax_conformance_spec's "declares
        # every keyword argument each word's builder takes" inspects
        # `instance_method(:attribute).parameters` and only counts real
        # `key`/`keyreq` entries, so a `**kwargs` override made every named
        # argument syntax.bluebook declares for Aggregate.attribute
        # (default/optional/pattern/admits/...) look unanswered — a real
        # regression from the primitive-wrapper synthesis pass, not a
        # deliberate signature change. Fixed at the root, not by weakening
        # the spec.
        def attribute(name, type = String, as: nil, default: nil, optional: false, required: nil,
                      pattern: nil, admits: nil, logged: true, enum: nil)
          # Vendored fix, not (yet) upstream hecksagain (migration plan task
          # 4): apply the SAME inverted-form correction AttributeCollector#
          # attribute makes -- but BEFORE the primitive-wrapper check below,
          # not after. `super` (which is where the base correction actually
          # lived) runs LAST, so a bare `attribute App` (no `as:`) used to
          # reach the primitive check below still shaped `name: :App, type:
          # String` (the un-fixed positional default) -- String IS a
          # PRIMITIVE_CLASS, so it synthesised a wrapper VO NAMED "App",
          # colliding with a real hand-written `value_object "App"` a few
          # lines above it in the same file. See AttributeCollector.
          # resolve_inverted's own comment for why the check is reliable.
          name, type = AttributeCollector.resolve_inverted(name, type, as)
          type = AttributeCollector.normalize_boolean_alias(type)

          if PRIMITIVE_CLASSES.include?(type)
            type = synthesise_primitive_wrapper(name, type)
          elsif type.is_a?(ListOf) && PRIMITIVE_CLASSES.include?(type.type)
            type = ListOf.new(synthesise_primitive_wrapper(name, type.type))
          end
          super(name, type, as: as, default: default, optional: optional, required: required,
                pattern: pattern, admits: admits, logged: logged, enum: enum)
        end

        def synthesise_primitive_wrapper(name, primitive_class)
          wrapper_name = Naming.pascal(name)
          (@closed_sets ||= closed_sets) << IR::ValueObject.declare(
            name:       wrapper_name,
            attributes: [IR::Attribute.new(name: :value, type: primitive_class.name)]
          )
          wrapper_name
        end

        # Vendored addition, not (yet) upstream hecksagain: miette's
        # consciousness.bluebook (and others) declare NAMED, reusable
        # boolean predicates over the aggregate's own state --
        # `specification :in_lucid_rem do |body| body.state == "sleeping" && ... end`
        # -- distinct from a command's `given` (precondition) or a VO's
        # `invariant` (field rule): a specification belongs to the
        # AGGREGATE, named, presumably referenced elsewhere by name.
        # Stored structurally (captured via the same canonical-source
        # extraction `given`/`invariant` use) so the corpus boots; NOT
        # yet threaded into IR::Aggregate or referenceable by name
        # elsewhere -- a real, documented gap, not silently pretended
        # complete. TODO upstream via bin/evolve (migration plan task 7).
        attr_reader :specifications
        def specification(name, &predicate)
          canonical = Ports::Extraction.canonical(predicate)
          (@specifications ||= []) << { name: name.to_s, canonical: canonical, predicate: predicate }
        end

        # Vendored addition, not (yet) upstream hecksagain (migration plan
        # task 4): an AGGREGATE-level `invariant "description" do ... end`
        # — a whole-record rule (miette's mind/awareness/witness.bluebook:
        # "WitnessedMoments are append-only — never updated, never
        # deleted"), distinct from a value object's field-scoped
        # `invariant` (ValueObjectBuilder#invariant, which this mirrors)
        # and a command's `given` precondition. Same documented-gap shape
        # as `specification` right above: captured structurally so the
        # corpus boots, NOT yet threaded into IR::Aggregate or walked at
        # dispatch time — the corpus's own comment already says as much
        # ("once the runtime walks specifications + invariants in
        # production... until then, the antibody at commit time scans").
        # TODO upstream via bin/evolve (migration plan task 7).
        attr_reader :aggregate_invariants
        def invariant(description = "an invariant holds", &predicate)
          canonical = Ports::Extraction.canonical(predicate)
          (@aggregate_invariants ||= []) << { description: description, canonical: canonical, predicate: predicate }
        end
        # Vendored alias, not (yet) upstream hecksagain (migration plan
        # task 8): `rule` at the AGGREGATE level too, not just inside a
        # value_object (ValueObjectBuilder's own alias) -- bin-buddy's
        # add_on.bluebook writes a whole-aggregate `rule` the same way
        # macrophage.bluebook writes a whole-aggregate `invariant`.
        alias_method :rule, :invariant

        # Vendored no-op stub, not (yet) upstream hecksagain (migration
        # plan task 8): `validation :field, presence: true` -- a Rails-
        # style field-presence declaration (pigeoncoop.bluebook, 5
        # occurrences, always `presence: true`). Genuinely a field-level
        # invariant in spirit ("this field must always be present"), but
        # NOT synthesized as a real `invariant` call here : `invariant`'s
        # predicate is canonicalized from its OWN literal source text
        # (Ports::Extraction.canonical reads the block's actual bluebook-
        # file location), and a dynamically-built predicate standing in
        # for a `validation` call has no real source line to extract from
        # -- attempting it risked a subtly wrong canonical form rather
        # than an honest gap. Structurally captured so the file boots,
        # not enforced -- same documented-gap pattern as `gate`/
        # `success`/`failure` elsewhere in this migration. TODO upstream
        # via bin/evolve (migration plan task 7): a real field-presence
        # sugar, not a one-off stub.
        def validation(*) = nil

        # ORIGIN, not runtime identity — a concept adopted from a canonical
        # source (§28) names where it came from without that fact ever
        # touching `hecks_fqn`/dispatch. Captured raw, the same way
        # `attribute ..., default: { value: "small" }` captures a literal
        # Hash untouched — no re-parsing, no structure imposed beyond
        # "whatever the author wrote."
        def provenance(from:)
          @provenance = from
        end

        # WHICH UNCHANGING FACTS SAY WHICH ONE THIS IS — FIELDS, not whole value
        # objects. `identified_by { number.value }` names the scalar inside
        # AccountNumber ; an identity is a value, and serialising a value object
        # into one only worked while something downstream guessed at `values.first`.
        # Nothing guesses now.
        #
        # SEVERAL PATHS, and the identity is their JOIN, in declaration order :
        #
        #   identified_by do
        #     aggregate_id
        #     name.value
        #   end                        ->  "Pizzas::Order:PlaceOrder"
        #
        # One path is the ordinary case and reads exactly as it always did. More
        # than one is what a thing named BENEATH another needs : a command is not
        # named by `PlaceOrder`, which every chapter may spell, but by the
        # aggregate it belongs to AND that name. Nothing here is minted, so the
        # same declaration names the same record on every run.
        #
        # The block is never CALLED. Its source is read the same way a given's is
        # (Ports::Extraction), which is why `number.value` needs no method called
        # `number` to exist — the same reason `balance >= amount` works in a given.
        # The canonical form collapses the block's newlines to single spaces, so
        # the paths arrive here already separated and in the order written.
        def identified_by(&path)
          raise Malformed, "#{@name}.identified_by names no field" unless path

          paths = Ports::Extraction.canonical(path).to_s.split(" ").reject(&:empty?)
          raise Malformed, "#{@name}.identified_by names no field" if paths.empty?

          @identity_paths = paths
        end

        def reference_to(type, as: nil)
          target = Naming.demodulise(type)
          @reference_targets << target
          attribute(as || :"#{Naming.snake(target)}_id", IR::Reference.new(target))
        end

        # `has_many`, `has_one`, `belongs_to` — relationship vocabulary Hecks
        # already grew (README's cherry-pick note) but this DSL never
        # declared: a bluebook using one simply failed to load here. All
        # three are sugar over `reference_to`, differing
        # from its default only in the attribute name they mint : no `_id`
        # suffix (matching Hecks' own reading of them, not
        # `reference_to`'s own `_id` mint), and `has_many`'s target is the
        # SINGULAR of what was written (`has_many Invoices` points at Invoice).
        #
        # `has_many` keeps the EXISTING shape — a single reference, not a
        # list. `list_of(Reference<X>)` has no precedent anywhere in this IR :
        # `list_of` is checked everywhere as a list of VALUE OBJECTS
        # (the mutation and read paths alike). A real one-to-many is a
        # separate arc, not a rename of what
        # already parses.
        def has_many(type, as: nil)
          plural = Naming.demodulise(type)
          reference_to(Naming.singularize(plural), as: as || Naming.snake(plural).to_sym)
        end

        def has_one(type, as: nil)
          reference_to(type, as: as || Naming.snake(Naming.demodulise(type)).to_sym)
        end

        def belongs_to(type, as: nil) = has_one(type, as: as)

        def lifecycle(field, default:, &block)
          @lifecycle = LifecycleBuilder.build(field, default: default, &block)
        end

        def entity(name, &block)
          # A piece is declared IN this aggregate — its owner is stamped by
          # `IR::Aggregate#initialize`, once the aggregate exists. Its own
          # commands were given the piece as their owner when it was declared,
          # so the chain closes as chapter -> aggregate -> entity -> command.
          @entities << EntityBuilder.build(name, &block)
        end

        def query(name, &block)
          @queries << QueryBuilder.build(name, &block)
        end

        def policy(name, &block)
          reaction = PolicyBuilder.build(name, &block)
          reaction.aggregate = @name
          @policies << reaction
        end

        def value_object(name, &block)
          @value_objects << ValueObjectBuilder.build(name, &block)
        end

        # Vendored no-op stub, not (yet) upstream hecksagain (migration
        # plan task 8): `fixture "Name" do field value ... end` declared
        # INSIDE an aggregate block -- a THIRD fixture shape hecks_nursury
        # uses (328 occurrences, 32 files) besides its 312 dedicated
        # `.fixtures` files and BluebookBuilder's already-stubbed
        # bluebook-level `fixture "Name", on: "Aggregate" do ... end`
        # (deciderate). Same documented gap, same reasoning as that
        # sibling stub's own comment : the field-setter calls inside
        # (`name "House Battery Bank"`, `voltage 12.8`) have no real
        # receiver methods, one per the aggregate's own attribute names,
        # varying per file -- executing the block for real needs an
        # actual seeding mechanism (dispatch a Declare-equivalent at
        # boot, or write straight into the repository bypassing command
        # validation), a real design question not attempted here.
        # Accepted so the file boots ; block body captured via a tiny
        # inert `method_missing` receiver (field names vary too widely
        # per aggregate to enumerate), NOT stored or seeded anywhere.
        # TODO upstream via bin/evolve (migration plan task 7): a real
        # aggregate-fixture seeding mechanism, one design covering all
        # three shapes at once.
        class InlineFixtureFieldStub
          def method_missing(*, **, &) = nil
          def respond_to_missing?(*) = true
        end

        # Chris's call (2026-08-13, i745/i748): still ACCEPTED so the file
        # boots -- hecksagain has no fixture-LOADING mechanism at all yet
        # (not even the separate .fixtures-file convention bin-buddy relies
        # on ; real seeding is a genuine subsystem build, its own decision,
        # not attempted here) -- but no longer PURELY silent. A stderr
        # warning at parse time surfaces through every subcommand that
        # boots this file (validate, dispatch, behaviors alike), not just
        # one path -- cheaper than adding a whole new warnings-collection
        # channel to validate's own return shape, and it names the record
        # so a caller currently blind to this real content loss can grep
        # for it.
        def fixture(*args, **kwargs, &block)
          # The first positional arg is usually just the aggregate name
          # repeated (`fixture "Train", train_number: "MT-100", ...`), not a
          # distinguishing record id -- fold in the first non-`on:` kwarg
          # too, so two records on the same aggregate don't log identically.
          name   = args.first || "(unnamed)"
          detail = kwargs.reject { |k, _| k == :on }.first
          suffix = detail ? " #{detail[0]}=#{detail[1].inspect}" : ""
          $stderr.puts "[fixture] #{@name}.fixture(#{name.inspect}#{suffix}) accepted but NOT seeded " \
                       "-- hecksagain has no fixture-loading mechanism yet (i745/i748)"
          InlineFixtureFieldStub.new.instance_eval(&block) if block
        end

        def command(name, &block)
          # The verb is declared ON this aggregate — the owner `acts_on` answers
          # with — stamped by `IR::Aggregate#initialize` once the aggregate
          # exists. An ENTITY's commands take the entity as their owner instead,
          # at the entity's own declaration.
          @commands << CommandBuilder.build(name, owner: @name, &block)
        end

        def build
          # Vendored default, not (yet) upstream hecksagain: hecks_nursury
          # (375 files) declares no identified_by anywhere -- hecksagain
          # refuses an aggregate with none ("an aggregate says what it
          # is known by"). Falls back to the FIRST declared attribute's
          # own .value path rather than requiring a corpus-wide add-a-
          # line pass across every nursery domain -- either exactly
          # right (most nursery aggregates have one obviously-primary
          # field) or a visible wrong guess a real boot/dispatch
          # surfaces immediately, not a silent one. TODO upstream via
          # bin/evolve (migration plan task 7).
          if @identity_paths.empty? && attributes.any?
            @identity_paths = ["#{attributes.first.name}.value"]
          end

          resolve_bare_primitive_collisions

          seal_mutation_targets
          seal_query_targets
          seal_defaults

          ir = IR::Aggregate.new(
            name:          @name,
            description:   @description,
            attributes:    attributes,
            value_objects: @value_objects + closed_sets,
            commands:      @commands,
            identified_by: @identity_paths,
            lifecycle:     @lifecycle,
            entities:      @entities,
            queries:       @queries,
            policies:      @policies,
            reference_targets: @reference_targets,
            provenance:    @provenance
          )

          # After the IR exists, on purpose : a reference is declared IN the
          # aggregate, and the aggregate the IR graph knows is `ir`, not the
          # builder.
          stamp_references(ir)
          ir
        end

        # inline_description -- vendored addition, not (yet) upstream
        # hecksagain. hecks_conception writes `aggregate "Name", "desc" do
        # ... end` (a second positional string) rather than a
        # `description "desc"` call inside the block -- syntactic sugar
        # over the same information, so this just calls #description
        # first rather than duplicating what it does. TODO upstream via
        # hecksagain's own bin/evolve word-admission process (migration
        # plan task 7).
        def self.build(name, inline_description = nil, &block)
          builder = new(name)
          builder.description(inline_description) if inline_description
          builder.instance_eval(&block) if block
          builder.build
        end

        private

        # Vendored fix, not (yet) upstream hecksagain (migration plan task
        # 8): `synthesise_primitive_wrapper` (above) mints an auto-wrapper
        # VO named after a bare attribute's own field (PascalCase) with NO
        # check for whether an EXPLICIT, hand-written `value_object` of
        # that same name already exists elsewhere in the same aggregate,
        # for an entirely unrelated purpose. hecks_nursury's own
        # biology.bluebook: `Neuron`'s bare `attribute :neurotransmitter,
        # String` (a plain field on the neuron itself) shares its
        # PascalCased name with a hand-written `value_object
        # "Neurotransmitter" do ... end` (an embedded shape `Synapse`'s
        # OWN `neurotransmitter` field uses) -- same English word, two
        # unrelated, both CORRECTLY-authored concepts, an ordinary domain-
        # modeling coincidence, not a corpus bug to rewrite around. This
        # is the same FAMILY of bug as the earlier "App" collision
        # (AttributeCollector's own comment) but not the same CAUSE --
        # that one was a misparse ; this is two independently-intended
        # declarations. And unlike the App case, attribute-call-time could
        # not have caught this even with a perfect check : the bare
        # attribute here is declared BEFORE the value_object it collides
        # with, so @value_objects is not yet populated when the wrapper is
        # synthesised. Deferred to build time on purpose, once
        # @value_objects holds everything the block declared. Resolved by
        # disambiguating the SYNTHESISED wrapper only -- the hand-written
        # VO's name is real authorial intent and is never touched -- and
        # re-pointing the bare attribute's own type string at the
        # disambiguated name, so both concepts keep their own, non-
        # colliding IR entry rather than the second Declare crashing the
        # boot. TODO upstream via bin/evolve (migration plan task 7).
        def resolve_bare_primitive_collisions
          return if closed_sets.empty?

          handwritten = @value_objects.map { |vo| vo.hecks_name.to_s }
          taken       = handwritten + closed_sets.map { |c| c.hecks_name.to_s }

          closed_sets.each do |synthesised|
            old_name = synthesised.hecks_name.to_s
            next unless handwritten.include?(old_name)

            new_name = "#{old_name}Value"
            new_name = "#{old_name}Value#{taken.count(new_name) + 1}" while taken.include?(new_name)
            taken << new_name
            synthesised.hecks_name = new_name

            attributes.each_with_index do |attr, i|
              next unless attr.type.to_s == old_name

              attributes[i] = IR::Attribute.new(
                name: attr.name, type: new_name, list: attr.list?, default: attr.default,
                optional: attr.optional?, pattern: attr.pattern, admits: attr.admits,
                logged: attr.logged
              )
            end
          end
        end

        # Every reference is told which IR::Aggregate declares it, so it can
        # find the chapter and resolve its target.
        #
        # Stamped HERE, at build, rather than at `reference_to`, because a command
        # builder does not hold the aggregate and should not learn to. And
        # deliberately across every list that can carry one — a reference the walk
        # missed would resolve to nil, and `resolve_references` SKIPS a nil target,
        # so the guarantee would go quiet instead of going red. That is the exact
        # shape of the bug that let an Account belong to an unregistered customer
        # fourteen times over.
        def stamp_references(ir)
          reference_bearing_attributes.each { |attribute| attribute.type.declared_in = ir }
        end

        def reference_bearing_attributes
          lists = [attributes, *@commands.map(&:attributes), *@queries.map(&:attributes)]
          @entities.each do |entity|
            lists << entity.attributes
            lists.concat(entity.commands.map(&:attributes))
            lists.concat(entity.queries.map(&:attributes))
          end

          lists.flatten.select(&:reference?)
        end

        # A mutation must name a field the aggregate actually HAS.
        #
        # NOT moved to the language, and deliberately so. The language says only
        # `given("a mutation names a target") { !target.value.to_s.empty? }` —
        # non-emptiness — because saying more means reaching a list that lives on
        # a DIFFERENT root : a command's changes hang off Command, the fields they
        # name hang off Aggregate, and a given is a closed predicate over its own
        # state. Aggregate.Seal is the right shape and cannot see commands ; the
        # reference trick that rescued "attributes use value-object types" needs a
        # root to point at, and an aggregate's fields are a value-object list, not
        # roots. This is the second rule that cannot port for that reason — the
        # first is read-model uniqueness — and both wait on the same thing : a
        # quantifier, or fields promoted to roots.
        #
        # So it lives here, at build, where every declaration is present. Found by
        # writing `then_set :disputed_by` on CardPayment before the field existed :
        # it wrote into nothing, refused nothing, and every check stayed green.
        # A DEFAULT FILLS THE SHAPE IT IS DECLARED ON, or it fills nothing.
        #
        # `attribute :cover, one_of("covered", "open"), default: "open"` builds
        # cleanly and then refuses EVERY create at dispatch — "cover is a Cover,
        # pass its fields as an object" — because the value object wants its
        # fields and got a bare string. The bluebook is wrong at the line where
        # it is written and says so nowhere near it.
        #
        # It cost a corpus member 33 refusals out of 40 steps, with every gate
        # green throughout: the refusals were perfectly consistent, which is
        # consistency about nothing. `till.bluebook` has always had the right shape
        # — `default: { cents: 0 }`.
        #
        # A PRIMITIVE takes a scalar and a VALUE OBJECT takes its fields, so the
        # test is simply which one the type names. Nothing here guesses at the
        # keys: a default that is a Hash is left to `Value.for_attribute`, which
        # is where a wrong FIELD belongs.
        def seal_defaults
          shapes = @value_objects.map { |shape| shape.hecks_name.to_s }

          attributes.each do |attribute|
            next if attribute.default.nil? || attribute.default.is_a?(Hash)
            next unless shapes.include?(attribute.type.to_s)

            raise Malformed,
                  "#{@name}.#{attribute.name} defaults to #{attribute.default.inspect}, but " \
                  "#{attribute.type} is a value object — a default fills its FIELDS " \
                  "(default: { ... }), and a bare value refuses every create instead"
          end
        end

        def seal_mutation_targets
          known = attributes.map { |attribute| attribute.name.to_sym }
          known << @lifecycle.field.to_sym if @lifecycle

          @commands.each do |command|
            command.mutations.each do |mutation|
              next if known.include?(mutation.target.to_sym)

              raise Malformed,
                    "#{@name}.#{command.hecks_name} sets #{mutation.target}, which #{@name} " \
                    "never declares — a mutation into a field that does not exist " \
                    "writes nothing and refuses nothing"
            end
          end
        end

        # A query must ask about a field the aggregate actually HAS — the same
        # seal `then_set` gets, closing the same silence: a where over a field
        # nothing declares matches nothing and refuses nothing, forever, on
        # every adapter. Three more silences close with it. A dotted path may
        # reach through the value-object graph but must LAND on a scalar
        # member (QuerySpecification::FieldPath is the one walk every engine
        # now shares) — landing on a value object hands SQL a JSON object
        # where the reference interpreter unwraps a hash. An ordered
        # comparator (lt/gt/gte/lte) must land on a numeric leaf — over text
        # the reference interpreter quietly matches no rows while SQL
        # compares lexicographically. And a :symbol value must name one of
        # the query's own declared arguments, or it resolves to nil at
        # dispatch and matches nothing.
        ORDERED_COMPARATORS = %i[lt lte gt gte].freeze

        def seal_query_targets
          query_surfaces.each do |owner, fields, lifecycle, queries|
            queries.each do |query|
              query.wheres.each do |clause|
                seal_query_field(owner, query, fields, lifecycle, clause.field)
                seal_ordered_comparator(owner, query, fields, clause)
                seal_query_argument(owner, query, clause.value)
              end
              seal_query_field(owner, query, fields, lifecycle, query.order_by.field, ordering: true) if query.order_by
              seal_query_argument(owner, query, query.limit&.value)
              seal_query_argument(owner, query, query.offset&.value)
            end
          end
        end

        def query_surfaces
          [[@name, attributes, @lifecycle, @queries]] +
            @entities.map { |entity| ["#{@name}::#{entity.hecks_name}", entity.attributes, entity.lifecycle, entity.queries] }
        end

        def seal_query_field(owner, query, fields, lifecycle, field, ordering: false)
          name, *nested = field.to_s.split(".")
          attribute = fields.find { |candidate| candidate.name.to_s == name }
          return if nested.empty? && (attribute || lifecycle&.field.to_s == name)
          return if nested.any? && attribute && scalar_path?(attribute, nested)

          if nested.any? && attribute && resolves?(attribute, nested)
            raise Malformed,
                  "#{owner}.#{query.hecks_name} asks about #{field}, which lands on a " \
                  "value object, not a scalar — a dotted query path ends on a scalar " \
                  "member, or the engines answer it differently"
          end

          if nested.any? && QuerySpecification::HopPath.hop_head?(field, fields)
            # ORDER BY refuses a hop OUTRIGHT, right here — unlike a
            # WHERE hop (deferred below), this doesn't need the
            # target's shape to answer: an ask is ordered by what its
            # own answering rows hold, and a hop answers with a
            # candidate set, not a sort key (see Runtime::ReferenceHop).
            if ordering
              raise Malformed,
                    "#{owner}.#{query.hecks_name} orders by #{field}, which hops through " \
                    "a reference — an ask is ordered by what its own answering rows " \
                    "hold, and a hop answers with a candidate set, not a sort key"
            end

            # RECOGNISED HERE, CHECKED LATER. The head names one of
            # this aggregate's own references, which is answerable
            # now — a Reference knows its own target_name at
            # declaration. What it points AT is not: stamp_references
            # has already run by this point, but the chapter
            # (IR::Bluebook, and the owning aggregate's OWN place in
            # it) does not exist yet, so Reference#resolve would
            # answer nil for every target in the file, including ones
            # declared above this one. The tail, and whether the
            # target even exists, are BluebookBuilder's business —
            # see validate_query_hops!, which runs once the chapter is
            # real, for exactly the reason
            # validate_no_bidirectional_references! already gives for
            # living at that same later point.
            return
          end

          raise Malformed,
                "#{owner}.#{query.hecks_name} asks about #{field}, which #{owner} " \
                "never declares — a query over a field that does not exist " \
                "matches nothing and refuses nothing"
        end

        def seal_ordered_comparator(owner, query, fields, clause)
          return unless ORDERED_COMPARATORS.include?(clause.op.to_s.to_sym)

          name, *nested = clause.field.to_s.split(".")
          attribute = fields.find { |candidate| candidate.name.to_s == name }
          return if attribute &&
                    QuerySpecification::FieldPath.numeric?(attribute, nested) { |type| declared_value_object(type) }

          # A WHERE clause hopping through a reference with an ordered
          # comparator is legitimate ("client whose balance > 500") —
          # unlike ORDER BY (refused outright in seal_query_field, see
          # its own comment), a where-clause hop answers a real
          # candidate set either way, ordered or not. Deferred for the
          # same reason any other hop is: whether the tail is even
          # numeric is BluebookBuilder#validate_query_hops!'s question
          # to ask of the TARGET's shape, not this aggregate's own.
          return if nested.any? && QuerySpecification::HopPath.hop_head?(clause.field, fields)

          held = attribute ? "holds no number" : "is the lifecycle field, which holds text"
          raise Malformed,
                "#{owner}.#{query.hecks_name} compares #{clause.field} with #{clause.op}, " \
                "but #{clause.field} #{held} — an ordered comparison needs a numeric " \
                "field, and over anything else the adapters answer differently or not at all"
        end

        def seal_query_argument(owner, query, value)
          return unless value.is_a?(Symbol)
          return if query.attribute(value)

          raise Malformed,
                "#{owner}.#{query.hecks_name} resolves :#{value} from its arguments, " \
                "but declares no #{value} attribute — an argument that does not exist " \
                "resolves to nil and matches nothing"
        end

        def scalar_path?(attribute, nested)
          QuerySpecification::FieldPath.scalar_leaf?(attribute, nested) { |type| declared_value_object(type) }
        end

        def resolves?(attribute, nested)
          !QuerySpecification::FieldPath.leaf_attribute(attribute, nested) { |type| declared_value_object(type) }.nil?
        end

        def declared_value_object(type_name)
          (@value_objects + closed_sets).find { |shape| shape.hecks_name.to_s == type_name }
        end

      end
    end
  end
end
