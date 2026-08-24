module HecksPlayground
  module ValidationRules

    # HecksPlayground::ValidationRules::WorldConcerns
    #
    # Validation rules activated by the +world_concerns+ DSL keyword. Each rule
    # checks a specific ethical or governance concern (transparency, consent,
    # privacy, security) and only fires when its concern is declared on the domain.
    #
    # Rules are autoloaded and self-register via +HecksPlayground.register_validation_rule+.
    #
    #   HecksPlayground.bluebook "Health" do
    #     world_concerns :transparency, :consent, :privacy, :security
    #     # ... aggregates ...
    #   end
    #
    module WorldConcerns
      autoload :Transparency, "hecks_playground/validation_rules/world_concerns/transparency"
      autoload :Consent,      "hecks_playground/validation_rules/world_concerns/consent"
      autoload :Privacy,      "hecks_playground/validation_rules/world_concerns/privacy"
      autoload :Security,     "hecks_playground/validation_rules/world_concerns/security"
    end
  end
end
