# HecksPlayground::Chapters::Bluebook
#
# Self-describing domain definition for the Bluebook chapter. Models the
# DSL, IR, compiler, generators, validation, and tooling as aggregates.
# Organized into paragraphs: Structure, Behavior, Names, Tooling,
# Builders, Generators, GeneratorInternals, SpecGenerators,
# ValidationRules, DslInternals, Serializers, Visualizers, Ast,
# Migrations, Features.
#
#   domain = HecksPlayground::Chapters::Bluebook.definition
#   domain.aggregates.map(&:name)
#
module HecksPlayground
  module Chapters
    require_paragraphs(__FILE__)

    module Bluebook
      def self.summary = "Domain command language, model types, DSL builders, and code generators for HecksPlayground"

      def self.definition
        @definition ||= Chapters.definition_from_bluebook("bluebook")
      end
    end
  end
end
