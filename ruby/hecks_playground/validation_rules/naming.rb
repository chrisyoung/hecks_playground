module HecksPlayground
  module ValidationRules

    # HecksPlayground::ValidationRules::Naming
    #
    # Naming convention rules for domain validation. This module groups rules that
    # enforce proper naming throughout the domain model:
    #
    # - +CommandNaming+ -- commands must start with a verb (uses WordNet + custom verbs)
    # - +NameCollisions+ -- aggregate root names must not collide with value object/entity names
    # - +UniqueAggregateNames+ -- no duplicate aggregate names within a domain
    # - +ReservedNames+ -- rejects Ruby keywords as attribute names and invalid aggregate constants
    # - +ComputedNameCollisions+ -- computed attribute names must not collide with regular attribute names
    # - +GlossaryTermViolations+ -- warns (or errors in strict mode) when names use banned glossary terms
    # - +SafeIdentifierNames+ -- rejects dangerous characters that could cause injection in generated Go/Ruby code
    #
    # All rules are autoloaded and executed as part of +HecksPlayground.validate+.
    #
    module Naming
      autoload :CommandNaming,          "hecks_playground/validation_rules/naming/command_naming"
      autoload :NameCollisions,         "hecks_playground/validation_rules/naming/name_collisions"
      autoload :UniqueAggregateNames,   "hecks_playground/validation_rules/naming/unique_aggregate_names"
      autoload :ReservedNames,          "hecks_playground/validation_rules/naming/reserved_names"
      autoload :ComputedNameCollisions, "hecks_playground/validation_rules/naming/computed_name_collisions"
      autoload :GlossaryTermViolations, "hecks_playground/validation_rules/naming/glossary_term_violations"
      autoload :SafeIdentifierNames,    "hecks_playground/validation_rules/naming/safe_identifier_names"
      autoload :GlossaryEnforcement,   "hecks_playground/validation_rules/naming/glossary_enforcement"
    end
  end
end
