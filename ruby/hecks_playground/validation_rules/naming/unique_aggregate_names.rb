module HecksPlayground
  module ValidationRules
    module Naming

    # HecksPlayground::ValidationRules::Naming::UniqueAggregateNames
    #
    # Validates that all aggregate names within a domain are unique. Duplicate
    # aggregate names would cause constant collisions in the generated Ruby code
    # and ambiguity in command routing.
    #
    # Part of the ValidationRules::Naming group -- run by +HecksPlayground.validate+.
    #
    # Aggregates must have unique names within a domain.
    class UniqueAggregateNames < BaseRule
      # Checks for duplicate aggregate names and returns an error for each.
      #
      # @return [Array<String>] error messages listing each duplicated aggregate name
      def errors
        names = @domain.aggregates.map(&:name)
        duplicates = names.select { |n| names.count(n) > 1 }.uniq

        duplicates.map do |name|
          error("Duplicate aggregate name: #{name}",
            hint: "Rename one of the #{name} aggregates to distinguish them")
        end
      end
    end
    HecksPlayground.register_validation_rule(UniqueAggregateNames)
    end
  end
end
