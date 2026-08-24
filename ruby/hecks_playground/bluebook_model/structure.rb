module HecksPlayground
  module BluebookModel

    # HecksPlayground::BluebookModel::Structure
    #
    # Namespace for the structural building blocks of a domain model: domains,
    # aggregates, value objects, attributes, validations, invariants, scopes,
    # ports, read models, external systems, and actors.
    #
    # Part of the BluebookModel IR layer. Each child class is an intermediate
    # representation built by the DSL and consumed by generators.
    #
    # == Structural Hierarchy
    #
    # The classes in this namespace form a tree:
    #
    #   Structure::Domain              # root container holding aggregates and domain-level policies
    #     Structure::Aggregate         # DDD aggregate with commands, events, lifecycle, etc.
    #       Structure::Attribute       # typed field on any structure (aggregate, value object, entity, command, event)
    #       Structure::ValueObject     # immutable object defined by its attributes (no identity)
    #       Structure::Entity          # mutable sub-entity with identity (UUID)
    #       Structure::Validation      # attribute-level validation rule (presence, type, uniqueness)
    #       Structure::Invariant       # business rule that must always hold true
    #       Structure::Scope           # named query scope with static or callable conditions
    #       Structure::GateDefinition  # access-control port defining allowed methods per role
    #       Structure::Lifecycle       # state machine definition with transitions tied to commands
    #     Structure::ReadModel         # data view needed before issuing a command (Event Storming artifact)
    #     Structure::ExternalSystem    # third-party system outside the domain boundary
    #     Structure::Actor             # user role or persona that issues commands
    #
    # == Usage
    #
    # These classes are not instantiated directly by application code. They are
    # built by the DSL layer (BluebookBuilder, AggregateBuilder, etc.) when parsing
    # a domain definition, and then consumed by generators, the runtime, and
    # visualization tools.
    #
    #   # Typical flow:
    #   domain = HecksPlayground::DSL::BluebookBuilder.new("Pizzas") { ... }.build
    #   domain.class  # => HecksPlayground::BluebookModel::Structure::Domain
    #   domain.aggregates.first.class  # => HecksPlayground::BluebookModel::Structure::Aggregate
    #
    module Structure
      autoload :Domain,         "hecks_playground/bluebook_model/structure/domain"
      autoload :Aggregate,      "hecks_playground/bluebook_model/structure/aggregate"
      autoload :ValueObject,    "hecks_playground/bluebook_model/structure/value_object"
      autoload :Entity,         "hecks_playground/bluebook_model/structure/entity"
      autoload :Attribute,      "hecks_playground/bluebook_model/structure/attribute"
      autoload :Validation,     "hecks_playground/bluebook_model/structure/validation"
      autoload :Invariant,      "hecks_playground/bluebook_model/structure/invariant"
      autoload :Derivation,     "hecks_playground/bluebook_model/structure/derivation"
      autoload :Scope,          "hecks_playground/bluebook_model/structure/scope"
      # GateDefinition lives in hecksagon
      autoload :ReadModel,      "hecks_playground/bluebook_model/structure/read_model"
      autoload :ExternalSystem, "hecks_playground/bluebook_model/structure/external_system"
      autoload :Actor,          "hecks_playground/bluebook_model/structure/actor"
      autoload :Lifecycle,        "hecks_playground/bluebook_model/structure/lifecycle"
      autoload :StateTransition, "hecks_playground/bluebook_model/structure/state_transition"
      autoload :Reference,         "hecks_playground/bluebook_model/structure/reference"
      autoload :ComputedAttribute, "hecks_playground/bluebook_model/structure/computed_attribute"
      autoload :Paragraph,         "hecks_playground/bluebook_model/structure/paragraph"
      autoload :BluebookStructure, "hecks_playground/bluebook_model/structure/bluebook_structure"
      autoload :Fixture,           "hecks_playground/bluebook_model/structure/fixture"
      autoload :TestSuite,         "hecks_playground/bluebook_model/structure/test_suite"
      autoload :Test,              "hecks_playground/bluebook_model/structure/test"
      autoload :TestSetup,         "hecks_playground/bluebook_model/structure/test"
      autoload :View,              "hecks_playground/bluebook_model/structure/view"
    end
  end
end
