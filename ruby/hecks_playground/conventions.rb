# HecksPlayground::Conventions
#
# Naming helpers and data contracts for cross-target code generation.
# Merged from hecks_playground_templating/. Provides short aliases on the HecksPlayground module
# so consumers don't need verbose includes or qualified names.
#
# Bootstrap: NamingHelpers must load before chapter system since builders
# include NamingHelpers.
require "hecks_playground/conventions/naming_helpers"

# Load all contract files
Dir[File.join(__dir__, "conventions", "*_contract.rb")].sort.each { |f| require f }
  # Short aliases — use these instead of the full HecksPlayground::Conventions:: path

module HecksPlayground
  Names = Conventions::Names
  NamingHelpers = Conventions::NamingHelpers

  # Backward compat — HecksTemplating still works
  module ::HecksTemplating
    Names = HecksPlayground::Conventions::Names
    NamingHelpers = HecksPlayground::Conventions::NamingHelpers
    TypeContract = HecksPlayground::Conventions::TypeContract
    DisplayContract = HecksPlayground::Conventions::DisplayContract
    ViewContract = HecksPlayground::Conventions::ViewContract
    EventContract = HecksPlayground::Conventions::EventContract
    EventLogContract = HecksPlayground::Conventions::EventLogContract
    FormParsingContract = HecksPlayground::Conventions::FormParsingContract
    AggregateContract = HecksPlayground::Conventions::AggregateContract
    MigrationContract = HecksPlayground::Conventions::MigrationContract
    UILabelContract = HecksPlayground::Conventions::UILabelContract
    NamingContract = HecksPlayground::Conventions::Names
    CommandContract = HecksPlayground::Conventions::CommandContract
    RouteContract = HecksPlayground::Conventions::RouteContract
  end

  # HecksPlayground::Contracts
  #
  # Runtime registry that maps short contract names to their module implementations.
  #
  module Contracts
    @registry = {}
    def self.register(name, contract) = @registry[name.to_sym] = contract
    def self.for(name) = @registry[name.to_sym]
    def self.registered = @registry.keys
  end

  Contracts.register(:types,      Conventions::TypeContract)
  Contracts.register(:display,    Conventions::DisplayContract)
  Contracts.register(:views,      Conventions::ViewContract)
  Contracts.register(:events,     Conventions::EventContract)
  Contracts.register(:event_log,  Conventions::EventLogContract)
  Contracts.register(:forms,      Conventions::FormParsingContract)
  Contracts.register(:aggregates, Conventions::AggregateContract)
  Contracts.register(:naming,     Conventions::Names)
  Contracts.register(:migrations, Conventions::MigrationContract)
  Contracts.register(:ui_labels,  Conventions::UILabelContract)
  Contracts.register(:commands,   Conventions::CommandContract)
  Contracts.register(:routes,     Conventions::RouteContract)
  Contracts.register(:dispatch,   Conventions::DispatchContract)
  Contracts.register(:extensions, Conventions::ExtensionContract)
end
