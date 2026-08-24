module HecksPlayground
  module Generators
    # HecksPlayground::Generators::Infrastructure
    #
    # Parent module for infrastructure generators. Autoloads generators for
    # repository ports, memory adapters, autoload entry points, RSpec specs,
    # and the top-level domain gem generator (DomainGemGenerator). Part of
    # the Generators layer, consumed by DomainGemGenerator and InMemoryLoader.
    #
    # == Subcomponents
    #
    # - +PortGenerator+ -- generates repository port interfaces (abstract modules with
    #   +NotImplementedError+ stubs for +find+, +save+, +delete+)
    # - +MemoryAdapterGenerator+ -- generates in-memory repository implementations
    #   that store aggregates in a +Hash+, including a +query+ method for filtering
    # - +AutoloadGenerator+ -- generates the +autoload+ entry point file and
    #   per-aggregate autoload declarations for value objects and entities
    # - +SpecGenerator+ -- generates behavioral RSpec specs for aggregates, commands,
    #   events, value objects, and entities
    # - +SpecHelpers+ -- shared private helpers for building example arguments and
    #   values in generated specs
    # - +DomainGemGenerator+ -- orchestrates the full gem generation pipeline,
    #   delegating to all other generators and writing files to disk
    #
    module Infrastructure
      autoload :PortGenerator,          "hecks_playground/generators/infrastructure/port_generator"
      autoload :MemoryAdapterGenerator, "hecks_playground/generators/infrastructure/memory_adapter_generator"
      autoload :AutoloadGenerator,      "hecks_playground/generators/infrastructure/autoload_generator"
      autoload :SpecGenerator,          "hecks_playground/generators/infrastructure/spec_generator"
      autoload :SpecHelpers,            "hecks_playground/generators/infrastructure/spec_helpers"
      autoload :DomainGemGenerator,     "hecks_playground/generators/infrastructure/domain_gem_generator"
      autoload :FrameworkGemGenerator, "hecks_playground/generators/infrastructure/framework_gem_generator"
      autoload :SelfHostDiff,          "hecks_playground/generators/infrastructure/self_host_diff"
      autoload :RuntimeGenerator,      "hecks_playground/generators/infrastructure/runtime_generator"
      autoload :RepositoryWiringGenerator, "hecks_playground/generators/infrastructure/runtime_generator/repository_wiring_generator"
      autoload :PortWiringGenerator,       "hecks_playground/generators/infrastructure/runtime_generator/port_wiring_generator"
      autoload :SubscriberWiringGenerator, "hecks_playground/generators/infrastructure/runtime_generator/subscriber_wiring_generator"
      autoload :PolicyWiringGenerator,     "hecks_playground/generators/infrastructure/runtime_generator/policy_wiring_generator"
      autoload :ServiceWiringGenerator,    "hecks_playground/generators/infrastructure/runtime_generator/service_wiring_generator"
      autoload :WorkflowWiringGenerator,   "hecks_playground/generators/infrastructure/runtime_generator/workflow_wiring_generator"
      autoload :SagaWiringGenerator,       "hecks_playground/generators/infrastructure/runtime_generator/saga_wiring_generator"
    end
  end
end
