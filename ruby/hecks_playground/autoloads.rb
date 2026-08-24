# = HecksPlayground Autoloads
#
# Central autoload registry mapping every HecksPlayground module to its source file.
# This is an infrastructure-level file required by +lib/hecks_playground.rb+ to enable
# lazy loading of all framework components.
#
# Autoloads are organized into logical groups:
# - *Mixins* -- Modules included into generated domain classes (+Command+,
#   +Model+, +Query+, +Specification+)
# - *Framework plumbing* -- Core utilities, configuration, CLI, workshop,
#   versioning, and migrations
# - *Domain tools* -- Validator, connections, glossary, visualizer, serializer
# - *ValidationRules* -- Individual validation rule classes for domain linting
# - *BluebookModel* -- Structure and behavior types (aggregates, entities,
#   value objects, commands, events)
# - *DSL* -- Builder classes for the domain definition DSL
# - *Generators* -- Code generators for domain classes, SQL adapters, and
#   infrastructure scaffolding
# - *EventStorm* -- Parsers and builders for importing event storming artifacts
# - *Runtime components* -- Runtime, ports (commands, queries, repository,
#   event bus, queue), workflow executor, and view binding
# - *HTTP* -- Rack-based domain server, RPC server, route builder, and
#   OpenAPI/JSON Schema generators
#
# Connection-specific autoloads (HTTP, MCP, SQL, CLI) live in their respective
# top-level entry points (+hecks_playground_serve+, +hecks_playground_ai+, +hecks_playground_persist+,
# +hecks_playground_cli+).
#
module HecksPlayground
  # Mixins (included into generated aggregate/entity/value object classes)
  autoload :Command,        "hecks_playground/mixins/command"
  autoload :Model,          "hecks_playground/mixins/model"
  autoload :Query,          "hecks_playground/mixins/query"
  autoload :Specification,  "hecks_playground/mixins/specification"

  autoload :Utils,          "hecks_playground/utils"
  autoload :VERSION,        "hecks_playground/version"
  autoload :Configuration,  "hecks_playground/runtime/configuration"
  autoload :CLI,            "hecks_playground_cli/cli"
  # autoload :Workshop -- loaded by chapter system
  autoload :Versioner,      "hecks_playground/bluebook/versioner"
  autoload :Migrations,     "hecks_playground/bluebook/migrations"


  # Domain tools
  autoload :Validator,         "hecks_playground/bluebook/validator"
  autoload :BluebookConnections, "hecks_playground/bluebook/connections"
  autoload :BluebookGlossary,    "hecks_playground/bluebook/glossary"
  autoload :LlmsGenerator,     "hecks_playground/bluebook/llms_generator"
  autoload :BluebookVisualizer,  "hecks_playground/bluebook/visualizer"
  autoload :ContextMapGenerator, "hecks_playground/bluebook/context_map_generator"
  autoload :DslSerializer,     "hecks_playground/bluebook/dsl_serializer"
  autoload :DomainVersioning,  "hecks_playground/domain_versioning"
  autoload :FlowGenerator,     "hecks_playground/bluebook/flow_generator"

  # = HecksPlayground::ValidationRules
  #
  # Namespace for individual domain validation rule classes. Each rule
  # implements a +call(domain)+ method that returns an array of warning
  # or error messages. Rules are run by {HecksPlayground::Validator} to lint a
  # domain definition.
  module ValidationRules
    autoload :ValidationMessage, "hecks_playground/validation_rules/validation_message"
    autoload :BaseRule,    "hecks_playground/validation_rules/base_rule"
    autoload :Naming,      "hecks_playground/validation_rules/naming"
    autoload :References,  "hecks_playground/validation_rules/references"
    autoload :Structure,   "hecks_playground/validation_rules/structure"
    autoload :WorldConcerns, "hecks_playground/validation_rules/world_concerns"
  end

  # = HecksPlayground::BluebookModel
  #
  # Namespace for the domain model type system. Contains two sub-namespaces:
  # - +Structure+ -- Structural types: Domain, Aggregate, Entity, ValueObject,
  #   Attribute, ReadModel, Port, DomainService
  # - +Behavior+ -- Behavioral types: Command, Event, Policy, Lifecycle,
  #   Workflow, Guard, Invariant
  module BluebookModel
    autoload :Behavior,                "hecks_playground/bluebook_model/behavior"
    autoload :Structure,               "hecks_playground/bluebook_model/structure"
    autoload :Names,                   "hecks_playground/bluebook_model/names"
    autoload :PredicateSource,         "hecks_playground/bluebook_model/predicate_source"
    autoload :SubscriberRegistration,  "hecks_playground/bluebook_model/subscriber_registration"
  end

  # = HecksPlayground::DSL
  #
  # Namespace for the domain definition DSL builder classes. Each builder
  # provides a block-based API for constructing one type of domain model
  # element. Builders are used internally by {HecksPlayground::BluebookBuilderMethods}
  # and by the +HecksPlayground.build+ entry point.
  module DSL
    autoload :AttributeCollector, "hecks_playground/dsl/attribute_collector"
    autoload :BluebookBuilder,    "hecks_playground/dsl/bluebook_builder"
    autoload :AggregateBuilder,   "hecks_playground/dsl/aggregate_builder"
    autoload :ValueObjectBuilder, "hecks_playground/dsl/value_object_builder"
    autoload :EntityBuilder,      "hecks_playground/dsl/entity_builder"
    autoload :CommandBuilder,     "hecks_playground/dsl/command_builder"
    autoload :PolicyBuilder,       "hecks_playground/dsl/policy_builder"
    autoload :AggregateRebuilder,  "hecks_playground/dsl/aggregate_rebuilder"
    # GateBuilder lives in hecksagon, not bluebook DSL
    autoload :ServiceBuilder,      "hecks_playground/dsl/service_builder"
    autoload :LifecycleBuilder,    "hecks_playground/dsl/lifecycle_builder"
    autoload :ReadModelBuilder,    "hecks_playground/dsl/read_model_builder"
    autoload :WorkflowBuilder,     "hecks_playground/dsl/workflow_builder"
    autoload :BranchBuilder,       "hecks_playground/dsl/workflow_builder"
    autoload :StepCollector,       "hecks_playground/dsl/workflow_builder"
    autoload :ProcessManagerBuilder, "hecks_playground/dsl/process_manager_builder"
    autoload :CadenceBuilder,        "hecks_playground/dsl/cadence_builder"
    autoload :BlockGrammarBuilder,   "hecks_playground/dsl/block_grammar_builder"
  end

  # = HecksPlayground::Generators
  #
  # Namespace for code generators. Generators produce Ruby source files,
  # SQL schemas, and infrastructure scaffolding from domain definitions.
  # - +Domain+ -- Generates aggregate, entity, value object, and command classes
  # - +SQL+ -- Generates Sequel-based repository adapters and migration files
  # - +Infrastructure+ -- Generates gemspec, spec_helper, and project scaffolding
  module Generators
    autoload :Domain,         "hecks_playground/generators/domain"
    autoload :SQL,            "hecks_playground_persist"
    autoload :Infrastructure, "hecks_playground/generators/infrastructure"
  end

  require "hecks_playground/generators/registry"

  # = HecksPlayground::EventStorm
  #
  # Namespace for event storming import tools. Parses event storm artifacts
  # (from text or YAML) and converts them into HecksPlayground domain definitions.
  # - +Parser+ -- Parses structured text event storm notation
  # - +YamlParser+ -- Parses YAML-formatted event storm files
  # - +BluebookBuilder+ -- Converts parsed event storm data into domain objects
  # - +DslGenerator+ -- Generates HecksPlayground DSL source code from parsed data
  # - +Result+ -- Value object wrapping the parse result
  module EventStorm
    autoload :Parser,        "hecks_playground/event_storm/parser"
    autoload :YamlParser,    "hecks_playground/event_storm/yaml_parser"
    autoload :BluebookBuilder, "hecks_playground/event_storm/domain_builder"
    autoload :DslGenerator,  "hecks_playground/event_storm/dsl_generator"
    autoload :Result,        "hecks_playground/event_storm/result"
  end

  # Runtime and port components
  autoload :DryRunResult,      "hecks_playground/dry_run_result"
  # autoload :Runtime -- loaded by chapter system
  # autoload :Application -- loaded by chapter system
  # PortWiring is included directly in Runtime, no autoload needed
  autoload :AttachmentMethods, "hecks_playground/runtime/attachment_methods"
  autoload :EventBus,          "hecks_playground/ports/event_bus/event_bus"
  # FilteredEventBus, CrossDomainQuery, CrossDomainView → hecks_playground_multidomain
  autoload :Queue,             "hecks_playground/ports/queue"
  autoload :GateEnforcer,      "hecks_playground/runtime/gate_enforcer"
  autoload :Persistence,       "hecks_playground/ports/repository"
  autoload :Querying,          "hecks_playground/ports/queries"
  autoload :Commands,          "hecks_playground/ports/commands"
  autoload :AggregateDescriber, "hecks_playground/runtime/aggregate_describer"
  autoload :Introspection,     "hecks_playground/runtime/introspection"
  autoload :Versioning,        "hecks_playground/runtime/versioning"
  autoload :ViewBinding,       "hecks_playground/runtime/view_binding"
  autoload :WorkflowExecutor,  "hecks_playground/runtime/workflow_executor"

  # = HecksPlayground::HTTP
  #
  # Namespace for HTTP-related components. Provides Rack-based servers for
  # exposing domains over REST and JSON-RPC, plus OpenAPI and JSON Schema
  # generators for documentation.
  module HTTP
    autoload :BluebookServer,       "hecks_playground/extensions/serve/domain_server"
    autoload :MultiBluebookServer,  "hecks_playground/extensions/serve/multi_domain_server"
    autoload :RpcServer,          "hecks_playground/extensions/serve/rpc_server"
    autoload :RouteBuilder,       "hecks_playground/extensions/serve/route_builder"
    autoload :OpenapiGenerator,   "hecks_playground/generators/docs/openapi_generator"
    autoload :RpcDiscovery,       "hecks_playground/generators/docs/rpc_discovery"
    autoload :JsonSchemaGenerator,  "hecks_playground/generators/docs/json_schema_generator"
    autoload :TypescriptGenerator,  "hecks_playground/generators/docs/typescript_generator"
  end
end
