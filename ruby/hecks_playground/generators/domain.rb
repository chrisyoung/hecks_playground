module HecksPlayground
  module Generators
    # HecksPlayground::Generators::Domain
    #
    # Parent module for domain artifact generators. Autoloads generators for
    # aggregates, value objects, commands, events, policies, query classes,
    # and query object modules. Part of the Generators layer, consumed by
    # DomainGemGenerator and InMemoryLoader to produce domain code.
    #
    # Each generator follows the same pattern: initialize with a domain model
    # object and a +domain_module+ name, then call +#generate+ to produce a
    # Ruby source string. The generated code is either written to disk (gem
    # generation) or evaluated in-memory (playground/session).
    #
    # == Available Generators
    #
    # - +AggregateGenerator+ -- aggregate root classes with HecksPlayground::Model
    # - +ValueObjectGenerator+ -- frozen, immutable value objects with value equality
    # - +EntityGenerator+ -- mutable sub-entities with identity-based equality
    # - +CommandGenerator+ -- CQRS command classes (create/update) with event emission
    # - +EventGenerator+ -- frozen domain event classes with timestamps
    # - +PolicyGenerator+ -- guard and reactive policy classes
    # - +QueryGenerator+ -- query classes with +call+ methods
    # - +QueryObjectGenerator+ -- query modules with +by_<attr>+ finder methods
    # - +SubscriberGenerator+ -- event subscriber classes
    # - +SpecificationGenerator+ -- specification pattern classes with +satisfied_by?+
    # - +LifecycleGenerator+ -- state machine classes with transitions and predicates
    # - +ServiceGenerator+ -- domain service classes orchestrating cross-aggregate logic
    # - +WorkflowGenerator+ -- multi-step workflow classes with conditional branching
    # - +ViewGenerator+ -- CQRS read model (view) classes with event projections
    #
    # == Usage
    #
    #   Domain::AggregateGenerator.new(agg, domain_module: "PizzasDomain").generate
    #
    module Domain
      autoload :AggregateGenerator,    "hecks_playground/generators/domain/aggregate_generator"
      autoload :ValueObjectGenerator,  "hecks_playground/generators/domain/value_object_generator"
      autoload :EntityGenerator,       "hecks_playground/generators/domain/entity_generator"
      autoload :CommandGenerator,      "hecks_playground/generators/domain/command_generator"
      autoload :EventGenerator,        "hecks_playground/generators/domain/event_generator"
      autoload :PolicyGenerator,       "hecks_playground/generators/domain/policy_generator"
      autoload :QueryGenerator,        "hecks_playground/generators/domain/query_generator"
      autoload :QueryObjectGenerator,  "hecks_playground/generators/domain/query_object_generator"
      autoload :SubscriberGenerator,      "hecks_playground/generators/domain/subscriber_generator"
      autoload :SpecificationGenerator,  "hecks_playground/generators/domain/specification_generator"
      autoload :LifecycleGenerator,     "hecks_playground/generators/domain/lifecycle_generator"
      autoload :ServiceGenerator,       "hecks_playground/generators/domain/service_generator"
      autoload :WorkflowGenerator,      "hecks_playground/generators/domain/workflow_generator"
      autoload :ViewGenerator,          "hecks_playground/generators/domain/view_generator"
    end
  end
end
