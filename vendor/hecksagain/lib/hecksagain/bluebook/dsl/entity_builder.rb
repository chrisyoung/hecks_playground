module Hecksagain
  module Bluebook
    module DSL
      class EntityBuilder
        include AttributeCollector

        def initialize(name)
          @name     = name
          @commands = []
          @queries  = []
        end

        def description(value) = @description = value

        # A PIECE is known by a field, not by a whole value object.
        # `identified_by { sequence.value }` names the SCALAR inside it, which
        # is what an id actually is — a LedgerEntry is entry 3, not entry
        # {"value":3}. The block is recovered through the same extraction
        # port an aggregate's is, so the two constructs cannot drift apart in
        # how they spell an identity — INCLUDING the several-path form, which a
        # piece may say for the same reason a head may.
        def identified_by(&path)
          raise Malformed, "#{@name}.identified_by names no field" unless path

          paths = Ports::Extraction.canonical(path).to_s.split(" ").reject(&:empty?)
          raise Malformed, "#{@name}.identified_by names no field" if paths.empty?

          @identity_paths = paths
        end

        # THE SAME FIELD AggregateBuilder's OWN reference_to BUILDS — a
        # piece can hold a reference to another root exactly the way its
        # own head can (Card.assignee_id, a Team's own id), just never
        # to another PIECE, since there's no cross-piece addressing
        # anywhere in this language to resolve one against. `optional:`
        # exists here too — a piece free to point at one of several
        # possible targets, never more than one at a time, same as an
        # aggregate's own sibling `reference_to` calls. Does NOT
        # register the target in the owning aggregate's own
        # `reference_targets` (the bidirectional-relationship list
        # `bluebook_builder.rb` builds for docs) — `IR::Entity` has no
        # such reader to populate. A real, small, deliberately deferred
        # gap; nothing about dispatch, hydration, or querying needs it.
        def reference_to(type, as: nil, optional: false)
          target = Naming.demodulise(type)
          attribute(as || :"#{Naming.snake(target)}_id", IR::Reference.new(target), optional: optional)
        end

        def command(name, &block)
          @commands << CommandBuilder.build(name, owner: @name, &block)
        end

        def query(name, &block)
          @queries << QueryBuilder.build(name, &block)
        end

        def lifecycle(field, default:, &block)
          @lifecycle = LifecycleBuilder.build(field, default: default, &block)
        end

        def build
          IR::Entity.declare(
            name:          @name,
            description:   @description,
            identified_by: @identity_paths,
            attributes:    attributes,
            commands:      @commands,
            queries:       @queries,
            lifecycle:     @lifecycle
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
