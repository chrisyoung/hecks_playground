# = HecksPlayground::Chapters::Runtime
#
# Self-describing chapter for the HecksPlayground runtime layer. Covers the
# runtime container, command/event dispatch, ports, mixins, event
# sourcing, sagas, workflows, and domain versioning.
#
#   domain = HecksPlayground::Chapters::Runtime.definition
#   domain.aggregates.map(&:name)
#
module HecksPlayground
  module Chapters
    require_paragraphs(__FILE__)

    module Runtime
      def self.summary = "Core kernel of the HecksPlayground hexagonal DDD framework"

      def self.definition
        @definition ||= Chapters.definition_from_bluebook("runtime")
      end
    end
  end
end
