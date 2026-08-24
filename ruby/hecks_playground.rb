require "json"
require "date"
require "ostruct"

JSON::Validator.use_multi_json = false if defined?(JSON::Validator)

# Bootstrap — must load before chapter system (errors, naming, autoloads,
# registries). These define the module infrastructure that Chapters and
# BluebookBuilder depend on, so they cannot be chapter-driven.
require "hecks_playground/errors"
require "hecks_playground/errors/shell_adapter_error"
require "hecks_playground/errors/llm_adapter_error"
require "hecks_playground/conventions"
require "hecks_playground/autoloads"

# Module infrastructure — DSL, registries, discovery
require "hecks_playground/registry"
require "hecks_playground/set_registry"
require "hecks_playground/module_dsl"
require "hecks_playground/core_extensions"
require "hecks_playground/registries/extension_registry"
require "hecks_playground/registries/capability_registry"
require "hecks_playground/registries/bluebook_registry"
require "hecks_playground/bluebook/event_storm_importer"
require "hecks_playground/registries/cross_domain"
require "hecks_playground/registries/thread_context"
require "hecks_playground/registries/target_registry"
require "hecks_playground/registries/adapter_registry"
require "hecks_playground/registries/validation_registry"
require "hecks_playground/registries/dump_format_registry"
require "hecks_playground/registries/grammar_registry"

# Extend registry methods early — Bluebook's chapter loading needs them
module HecksPlayground
  extend ExtensionRegistryMethods
  extend CapabilityRegistryMethods
  extend BluebookRegistryMethods
  extend CrossDomainMethods
  extend ThreadContextMethods
  extend TargetRegistryMethods
  extend AdapterRegistryMethods
  extend ValidationRegistryMethods
  extend DumpFormatRegistryMethods
  extend GrammarRegistryMethods
end

# Chapter infrastructure — Chapters module defines require_paragraphs,
# load_chapter, etc. needed by all chapter registrations.
require "hecks_playground/chapters"
require "hecks_playground/chapter_aliases"
# Chapter selection — register all available chapters, then load
require "hecks_playground/chapter_loader"
require "hecks_playground/chapters/registry"

# Load chapters from HecksChapters file if present, otherwise load all.
# HecksChapters is a simple DSL file listing which chapters to include:
#
#   # HecksChapters
#   chapter :bluebook
#   chapter :runtime
#   chapter :hecksagon
#
# When no HecksChapters file exists, all chapters are loaded (default).
HecksPlayground::ChapterLoader.load_from_file || HecksPlayground.chapters(:all)

require "hecks_playground/bluebook/inspector"
require "hecks_playground/bluebook/builder_methods"
require "hecks_playground/bluebook/compiler"
require "hecks_playground/bluebook/visualizer_methods"
require "hecks_playground/bluebook/connections"
require "hecks_playground/registries/bluebook_registry"
require "hecks_playground/bluebook/event_storm_importer"

module HecksPlayground
  # Thread-local role for command authorization.
  # Set before dispatching role-restricted commands:
  #   HecksPlayground.current_role = "Customer"
  def self.current_role
    Thread.current[:_hecks_playground_current_role]
  end

  def self.current_role=(role)
    Thread.current[:_hecks_playground_current_role] = role
  end

  extend BluebookInspector
  extend BluebookBuilderMethods
  extend BluebookCompiler
  extend EventStormImporter
  extend BluebookVisualizerMethods
  extend Boot
  extend BootBluebook

  def self.configure(&block)
    @configuration = Configuration.new
    @configuration.instance_eval(&block)
    @configuration.boot! unless defined?(::Rails)
    @configuration
  end

  def self.configuration
    @configuration
  end

  # Load a domain from an IR object and return a booted Runtime. No filesystem
  # required -- uses InMemoryLoader to generate and eval source in memory.
  #
  #   runtime = HecksPlayground.load(domain)
  #   runtime = HecksPlayground.load(domain, event_bus: my_bus)
  #
  # @param domain [HecksPlayground::BluebookModel::Structure::Domain] the domain IR
  # @param force [Boolean] reload even if already cached (default false)
  # @param opts [Hash] extra options forwarded to Runtime (e.g. event_bus:)
  # @return [HecksPlayground::Runtime] a fully wired runtime with memory adapters
  def self.load(domain, force: false, **opts, &config)
    load_bluebook(domain, force: force)
    Runtime.new(domain, **opts, &config)
  end

  # Core build target — always available
  register_target(:ruby) { |domain, **opts| HecksPlayground.build(domain, **opts) }

  # Binary target — compiles HecksPlayground into a single bundled script
  register_target(:binary) { |_domain, **opts|
    require "hecks_playground/compiler"
    HecksPlayground::Compiler::BinaryCompiler.new.compile(output: opts[:output] || "hecks_playground_v0")
  }

  # Other targets (go, static, node, rails) self-register
  # when their chapters are loaded. See hecks_playground_targets/ for each.

  # Built-in dump formats
  register_dump_format(:schema, desc: "JSON Schema") { |domain, say:| require "hecks_playground_serve"; File.write("schema.json", JSON.pretty_generate(HecksPlayground::HTTP::JsonSchemaGenerator.new(domain).generate)); say.call("Dumped schema.json", :green) }
  register_dump_format(:swagger, desc: "OpenAPI 3.0") { |domain, say:| require "hecks_playground_serve"; File.write("openapi.json", JSON.pretty_generate(HecksPlayground::HTTP::OpenapiGenerator.new(domain).generate)); say.call("Dumped openapi.json", :green) }
  register_dump_format(:rpc, desc: "JSON-RPC discovery") { |domain, say:| require "hecks_playground_serve"; File.write("rpc_methods.json", JSON.pretty_generate(HecksPlayground::HTTP::RpcDiscovery.new(domain).generate)); say.call("Dumped rpc_methods.json", :green) }
  register_dump_format(:domain, desc: "domain gem") { |domain, say:| FileUtils.mkdir_p("domain"); say.call("Dumped domain gem to domain/#{File.basename(HecksPlayground.build(domain, output_dir: "domain"))}/", :green) }
  register_dump_format(:glossary, desc: "plain-English glossary") { |domain, say:| File.write("glossary.md", HecksPlayground::BluebookGlossary.new(domain).generate.join("\n") + "\n"); say.call("Dumped glossary.md", :green) }
  register_dump_format(:types, desc: "TypeScript types (.d.ts)") { |domain, say:| File.write("types.d.ts", HecksPlayground::HTTP::TypescriptGenerator.new(domain).generate); say.call("Dumped types.d.ts", :green) }
end
