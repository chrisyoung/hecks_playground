# HecksPlayground::Chapters::AI
#
# Self-describing chapter definition for the hecks_playground_ai gem.
# Enumerates every class and module under hecks_playground_ai/lib/ as
# aggregates with their key commands. Aggregates are split
# into focused sub-domain files under ai/ by concern.
#
#   domain = HecksPlayground::Chapters::AI.definition
#   domain.aggregates.map(&:name)
#   # => ["McpServer", "BluebookServer", "GovernanceGuard", ...]
#
require "bluebook"

require_relative "ai/mcp_tools"
require_relative "ai/mcp_server"
require_relative "ai/generation"
require_relative "ai/governance"

module HecksPlayground
  module Chapters
    module AI
      def self.summary = "AI integration for HecksPlayground"

      def self.definition
        HecksPlayground::DSL::BluebookBuilder.new("AI").tap { |b|
          Chapters.define_paragraphs(AI, b)
        }.build
      end
    end
  end
end
