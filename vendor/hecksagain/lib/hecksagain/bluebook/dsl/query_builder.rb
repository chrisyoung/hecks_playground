module Hecksagain
  module Bluebook
    module DSL
      class QueryBuilder
        include AttributeCollector
        include QuerySpecification::Common::DSL

        def initialize(name)
          @name   = name
          @wheres = []
          @index_hints = []
        end

        def description(value) = @description = value

        # `count` -- vendored addition, not (yet) upstream hecksagain
        # (migration plan task 4): see IR::Query's own comment.
        def count = @count = true

        # `median :field` -- vendored addition, not (yet) upstream
        # hecksagain (migration plan task 8): the SAME shape as `count`
        # (a scalar reduction over the filtered set, riding the where-
        # clauses that already apply), one field further -- deciderate's
        # own vision names this explicitly ("proves the grown query DSL
        # (aggregation)"; Consensus.median :estimate -- "the ESTIMATION
        # answer: the median estimate across a decision's submissions").
        # See Runtime::QueryInterpreter#call's own comment for the
        # evaluation side.
        def median(field) = @median_field = field.to_sym

        # `group_by :field` -- vendored addition, not (yet) upstream
        # hecksagain (migration plan task 8): a real, third scalar-
        # reduction shape alongside `count`/`median`, except it doesn't
        # reduce to ONE scalar -- it partitions the where-filtered set by
        # `field`'s value and tallies each partition, matching what
        # deciderate's own `PerPlayer`/leaderboard queries need ("the
        # player leaderboard : submissions tallied per player"). See
        # Runtime::QueryInterpreter#call's own comment for the
        # evaluation side.
        def group_by(field) = @group_by_field = field.to_sym

        # `scope_to :field` -- vendored NO-OP stub, not (yet) upstream
        # hecksagain (migration plan task 8): "injects a where(player ==
        # :actor) from the reserved `actor` kwarg the ACL/edge supplies"
        # (deciderate's own comment) -- real caller-identity-based row
        # authorization, a security-relevant feature this migration is
        # NOT rushing under time pressure (same discipline as `success`/
        # `failure`'s effect-family stubs -- structurally captured so the
        # file boots, never silently pretended enforced). TODO upstream
        # via bin/evolve (migration plan task 7): a real design pass on
        # how the runtime learns "who is calling."
        def scope_to(*) = nil

        # A query parameter naming another aggregate's own identity —
        # Item.ByOwner's `owner`, filtering by which Camper owns it,
        # not a free string that only happens to look like one. Only
        # ever the CROSS-reference half `CommandBuilder#reference_to`
        # has two of: a query has no root of its own to act on, so
        # there's no "acts on itself" case to distinguish here, only a
        # plain attribute typed as a reference —
        # `AttributeCollector#attribute` already handles an
        # `IR::Reference` type exactly like any other.
        def reference_to(type, as: nil, optional: false)
          demodulised = Naming.demodulise(type)
          attribute(as || :"#{Naming.snake(demodulised)}_id", IR::Reference.new(demodulised), optional: optional)
        end

        def build
          seal_cursor
          IR::Query.new(
            name:        @name,
            description: @description,
            attributes:  attributes,
            wheres:      @wheres,
            order_by:    @order_by,
            limit:       @limit,
            offset:      @offset,
            cursor:      @cursor,
            consistency: @consistency,
            freshness: @freshness,
            authorization: @authorization,
            null_semantics: @null_semantics,
            inspection: @inspection,
            index_hints: @index_hints || [],
            count: @count || false,
            median_field: @median_field,
            group_by_field: @group_by_field
          )
        end

        def self.build(name, &block)
          builder = new(name)
          builder.instance_eval(&block) if block
          builder.build
        end

        private

        # `cursor` parses, round-trips through the IR, and is read by nothing —
        # no interpreter (Memory, Sqlite, Postgres) ever applies it. Refusing
        # it here, rather than deleting the word, keeps the declared syntax
        # honest (the language still knows the shape) while refusing to let a
        # bluebook author believe cursor-based pagination actually happens.
        def seal_cursor
          return unless @cursor

          raise Malformed,
                "#{@name} declares cursor, but no interpreter implements cursor " \
                "pagination — use limit/offset instead"
        end
      end
    end
  end
end
