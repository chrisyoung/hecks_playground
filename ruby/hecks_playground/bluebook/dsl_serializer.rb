HecksPlayground::Chapters.load_aggregates(
  HecksPlayground::Bluebook::SerializersParagraph,
  base_dir: File.expand_path("dsl_serializer", __dir__)
)
  # HecksPlayground::DslSerializer
  #
  # Serializes a Domain IR back into DSL source code. The output is valid
  # Ruby that can be eval'd to reconstruct the domain.
  #
  #   DslSerializer.new(domain).serialize
  #   # => 'HecksPlayground.bluebook "Pizzas" do ...'
  #

module HecksPlayground
  class DslSerializer
    include TypeHelpers
    include RuleSerializer
    include BehaviorSerializer
    include AggregateSerializer

    def initialize(domain)
      @domain = domain
    end

    # @return [String] valid Ruby DSL source code
    def serialize
      lines = ["HecksPlayground.bluebook \"#{@domain.name}\" do"]
      lines << "  description \"#{@domain.description}\"" if @domain.description
      @domain.aggregates.each_with_index do |agg, i|
        lines << "" if i > 0
        lines.concat(serialize_aggregate(agg))
      end
      @domain.policies.each { |pol| lines.concat(serialize_domain_policy(pol)) }
      lines << "end"
      lines.join("\n") + "\n"
    end
  end
end
