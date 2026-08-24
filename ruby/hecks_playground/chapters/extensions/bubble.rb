# = HecksPlayground::Chapters::Extensions::BubbleChapter
#
# Self-describing sub-chapter for Bubble anti-corruption layer
# internals: aggregate mapper and context DSL host.
#
#   HecksPlayground::Chapters::Extensions::BubbleChapter.define(builder)
#
module HecksPlayground
  module Chapters
    module Extensions
      # HecksPlayground::Chapters::Extensions::BubbleChapter
      #
      # Bluebook sub-chapter for Bubble anti-corruption layer: aggregate mapper and context DSL host.
      #
      module BubbleChapter
        def self.define(b)
          b.aggregate "AggregateMapper", "Collects inbound/outbound field mappings for a single aggregate" do
            command("TranslateLegacy") { attribute :action, String; attribute :data, String }
            command("ReverseTranslate") { attribute :action, String; attribute :data, String }
          end

          b.aggregate "Context", "DSL host grouping aggregate mappers with translate and reverse entry points" do
            command("MapAggregate") { attribute :aggregate_name, String }
            command("Translate") { attribute :aggregate_name, String; attribute :action, String }
          end
        end
      end
    end
  end
end
