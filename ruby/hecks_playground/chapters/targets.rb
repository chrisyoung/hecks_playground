# HecksPlayground::Chapters::Targets
#
# Self-describing domain definition for the Targets chapter. The code
# generation layer models itself as a domain: Target represents a
# language backend, with paragraphs for Go, Node, and Ruby generators.
#
#   domain = HecksPlayground::Chapters::Targets.definition
#   domain.aggregates.map(&:name)
#
module HecksPlayground
  module Chapters
    require_paragraphs(__FILE__)

    module Targets
      def self.definition
        @definition ||= Chapters.definition_from_bluebook("targets")
      end
    end
  end
end
