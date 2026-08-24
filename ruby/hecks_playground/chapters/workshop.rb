# HecksPlayground::Chapters::Workshop
#
# Self-describing chapter definition for the hecks_playground_workshop gem.
# Enumerates every class and module under hecks_playground_workshop/lib/ as
# aggregates with their key commands.
#
#   domain = HecksPlayground::Chapters::Workshop.definition
#   domain.aggregates.map(&:name)
#   # => ["Workshop", "AggregateHandle", "CommandHandle", ...]
#
require "bluebook"

module HecksPlayground
  module Chapters
    require_paragraphs(__FILE__)

    module Workshop
      def self.summary = "Interactive REPL and MCP server for HecksPlayground"

      def self.definition
        @definition ||= Chapters.definition_from_bluebook("workshop")
      end
    end
  end
end
