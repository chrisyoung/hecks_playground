# = HecksAi
#
# MCP server and AI tools for HecksPlayground. Provides aggregate building,
# domain inspection, play mode, and build tools via MCP protocol.
# Also includes LLM-driven domain generation (HEC-102).
#
module HecksPlayground
  autoload :GovernanceGuard, "hecks_playground_ai/governance_guard"

  module AI
    autoload :McpServer,        "hecks_playground_ai/mcp_server"
    autoload :AggregateTools,   "hecks_playground_ai/aggregate_tools"
    autoload :BuildTools,       "hecks_playground_ai/build_tools"
    autoload :InspectTools,     "hecks_playground_ai/inspect_tools"
    autoload :PlayTools,        "hecks_playground_ai/play_tools"
    autoload :SessionTools,     "hecks_playground_ai/session_tools"
    autoload :DomainSerializer, "hecks_playground_ai/domain_serializer"
    autoload :DomainServer,     "hecks_playground_ai/domain_server"
    autoload :Connection,       "hecks_playground_ai/connection"
    autoload :TypeResolver,     "hecks_playground_ai/type_resolver"
    autoload :LlmClient,        "hecks_playground_ai/llm_client"
    autoload :BluebookBuilder,    "hecks_playground_ai/domain_builder"

    module Prompts
      autoload :DomainGeneration, "hecks_playground_ai/prompts/domain_generation"
      autoload :DomainToolSchema, "hecks_playground_ai/prompts/domain_tool_schema"
    end
  end
end
