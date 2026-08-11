module Hecksagain
  module Bluebook
    module IR
      def self.render_value(value) = QuerySpecification.render_value(value)

      # A query — an ASK, declared on an aggregate or on one of its entities.
      #
      # It crosses over as an INSTANCE rather than a class, and the reason is worth
      # stating because it is the boundary of the pattern. A query inherits its
      # whole body from `QuerySpecification::Common::Options` — `wheres`,
      # `order_by`, `limit`, `offset`, `cursor`, `consistency`, `freshness`,
      # `authorization`, `null_semantics`, `inspection`, `index_hints` — and those
      # are INSTANCE methods that the runtime and the SQLite adapter both read.
      # Hoisting the declaration onto a metaclass would put the identity and the
      # specification on opposite sides of the object.
      #
      # And nothing is lost, because `Class#name` was the only reason a construct
      # ever needed a second word for its own name. An instance answers `name`
      # truthfully. So a query gains an owner and an identity — which is what the
      # graph is for — and keeps the name it always had.
      class Query < QuerySpecification::Common::Options
        include Construct

        attr_reader :name, :description, :attributes, :count, :median_field, :group_by_field

        # `count:`, vendored addition not (yet) upstream hecksagain
        # (migration plan task 4): `query "Count" do count end`
        # (framework/web_debug/bluebook/web_debug.bluebook) -- a scalar
        # row-count ask, not a filtered record list. See
        # Runtime::QueryInterpreter#call's own comment for the
        # evaluation side (returns an Integer instead of an Array).
        #
        # `median_field:`, vendored addition not (yet) upstream
        # hecksagain (migration plan task 8): `median :field` -- the
        # same scalar-reduction shape as `count`, one field further. See
        # QueryBuilder#median's own comment.
        #
        # `group_by_field:`, vendored addition not (yet) upstream
        # hecksagain (migration plan task 8): `group_by :field` -- a
        # third aggregation shape, partition-and-tally rather than
        # reduce-to-one-scalar. See QueryBuilder#group_by's own comment.
        def initialize(name:, description: nil, attributes: [], wheres: [],
                       order_by: nil, limit: nil, offset: nil, cursor: nil,
                       consistency: nil, freshness: nil, authorization: nil, null_semantics: nil,
                       inspection: nil, index_hints: [], count: false, median_field: nil,
                       group_by_field: nil)
          null_semantics ||= QuerySpecification::Common::NullSemantics.default
          super(wheres: wheres, order_by: order_by, limit: limit, offset: offset, cursor: cursor,
                consistency: consistency, freshness: freshness, authorization: authorization,
                null_semantics: null_semantics, inspection: inspection, index_hints: index_hints)
          @name        = name.to_s
          @hecks_name  = @name
          @description = description
          @attributes  = attributes
          @count       = count
          @median_field = median_field
          @group_by_field = group_by_field
        end

        def attribute(named) = @attributes.find { |a| a.name == named.to_sym }

        def to_h
          {
            name:        @name,
            description: @description,
            attributes:  @attributes.map(&:to_h),
            wheres:      @wheres.map(&:to_h),
            order_by:    @order_by&.to_h,
            limit:       @limit&.to_h
          }.merge(extra_options_to_h)
        end
      end
    end
  end
end
