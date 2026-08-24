# HecksPlayground::Chapters::AI::GovernanceParagraph
#
# Paragraph defining AI governance aggregates: GovernanceGuard,
# GovernanceResult, and ConcernChecks.
#
#   HecksPlayground::Chapters.define_paragraphs(HecksPlayground::Chapters::AI, builder)
#
module HecksPlayground
  module Chapters
    module AI
      module GovernanceParagraph
        def self.define(b)
          b.aggregate "GovernanceGuard" do
            description "Entry-point agnostic governance checker against world concerns"
            command "Check"
          end

          b.aggregate "GovernanceResult" do
            description "Immutable result object from a governance check with violations and suggestions"
            command "Create" do
              attribute :violations, String
              attribute :suggestions, String
            end
          end

          b.aggregate "ConcernChecks" do
            description "Rule-based governance checks for transparency, consent, privacy, security"
            command "CheckTransparency"
            command "CheckConsent"
            command "CheckPrivacy"
            command "CheckSecurity"
          end
        end
      end
    end
  end
end
