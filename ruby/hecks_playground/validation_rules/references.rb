module HecksPlayground
  module ValidationRules

    # HecksPlayground::ValidationRules::References
    #
    # Reference integrity rules for domain validation. This module groups rules
    # that enforce correct usage of aggregate references:
    #
    # - +ValidReferences+ -- references must target existing aggregate roots (not value objects or entities)
    # - +NoBidirectionalReferences+ -- no mutual A->B and B->A references between aggregates
    # - +NoSelfReferences+ -- aggregates cannot reference themselves
    #
    # All rules are autoloaded and executed as part of +HecksPlayground.validate+.
    #
    module References
      autoload :ValidReferences,           "hecks_playground/validation_rules/references/valid_references"
      autoload :NoBidirectionalReferences, "hecks_playground/validation_rules/references/no_bidirectional_references"
      autoload :NoSelfReferences,          "hecks_playground/validation_rules/references/no_self_references"
      autoload :NoForeignKeyAttributes,    "hecks_playground/validation_rules/references/no_foreign_key_attributes"
      autoload :CrossContextReferences,    "hecks_playground/validation_rules/references/cross_context_references"
      autoload :SubdomainDirection,        "hecks_playground/validation_rules/references/subdomain_direction"
    end
  end
end
