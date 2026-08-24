# HecksServe
#
# HTTP and JSON-RPC server extension for HecksPlayground domains. Serves domains
# over REST and JSON-RPC via WEBrick. Includes OpenAPI, JSON Schema, and
# RPC discovery generators for API documentation.
#
# When registered, adds a +.serve(port:)+ singleton method to the domain
# module that starts a DomainServer on the specified port. The server
# auto-generates CRUD routes for all aggregates plus query endpoints.
#
# Sub-components:
# - {HecksPlayground::HTTP::DomainServer} -- WEBrick REST server with CORS
# - {HecksPlayground::HTTP::RpcServer} -- JSON-RPC 2.0 server
# - {HecksPlayground::HTTP::RouteBuilder} -- generates route definitions from aggregates
# - {HecksPlayground::Connections::HttpConnection} -- connection wrapper for boot blocks
#
# Future gem: hecks_playground_serve
#
#   require "hecks_playground_serve"
#   HecksPlayground::HTTP::DomainServer.new(domain, port: 3000).run
#
HecksPlayground.describe_extension(:http,
  description: "REST and JSON-RPC server with OpenAPI docs",
  adapter_type: :driving,
  config: { gate: { default: 9292, desc: "HTTP port" }, rpc: { default: false, desc: "Enable JSON-RPC mode" } },
  wires_to: :command_bus)

# Register the HTTP extension. Adds a +.serve+ method to the domain module
# that instantiates and runs a DomainServer.
#
# @param domain_mod [Module] the domain module constant (e.g. CatsDomain)
# @param domain [HecksPlayground::Domain] the parsed domain definition
# @param _runtime [HecksPlayground::Runtime] the runtime instance (unused)
HecksPlayground.register_extension(:http) do |domain_mod, domain, _runtime|
  domain_mod.define_singleton_method(:serve) do |port: 9292|
    HecksPlayground::HTTP::DomainServer.new(domain, gate: port).run
  end
end
  # HTTP server components for serving HecksPlayground domains over REST and JSON-RPC.

module HecksPlayground
  # HecksPlayground::HTTP
  #
  # Namespace for HTTP server components: DomainServer, RpcServer, RouteBuilder, and multi-domain support.
  #
  module HTTP
    autoload :DomainServer,       "hecks_playground/extensions/serve/domain_server"
    autoload :MultiDomainServer,  "hecks_playground/extensions/serve/multi_domain_server"
    autoload :RpcServer,          "hecks_playground/extensions/serve/rpc_server"
    autoload :RouteBuilder,       "hecks_playground/extensions/serve/route_builder"
    autoload :CommandBusPort,     "hecks_playground/extensions/serve/command_bus_port"
    autoload :OpenapiGenerator,   "hecks_playground/generators/docs/openapi_generator"
    autoload :RpcDiscovery,       "hecks_playground/generators/docs/rpc_discovery"
    autoload :JsonSchemaGenerator, "hecks_playground/generators/docs/json_schema_generator"
  end

  # Connection wrappers for boot-time wiring of external interfaces.
  module Connections
    autoload :HttpConnection, "hecks_playground/extensions/serve/connection"
  end
end
