# HecksPlayground::Chapters::Bluebook::VisualizersParagraph
#
# Paragraph covering visualizer classes: Mermaid diagram generators
# for domain structure, behavior flows, and port/adapter layouts.
#
#   HecksPlayground::Chapters::Bluebook::VisualizersParagraph.define(builder)
#
module HecksPlayground
  module Chapters
    module Bluebook
      module VisualizersParagraph
        def self.define(b)
          b.aggregate "DomainVisualizer", "Orchestrates Mermaid diagram generation for a domain" do
            command("VisualizeDomain") { attribute :domain_id, String }
          end

          b.aggregate "DomainVisualizerMethods", "Mixin extending HecksPlayground with the visualize method" do
            command("RegisterVisualizer") { attribute :domain_id, String }
          end

          b.aggregate "BehaviorDiagram", "Generates Mermaid diagrams for command and event flows" do
            command("GenerateBehaviorDiagram") { attribute :domain_id, String }
          end

          b.aggregate "PortDiagram", "Generates Mermaid diagrams for ports and adapters" do
            command("GeneratePortDiagram") { attribute :domain_id, String }
          end

          b.aggregate "StructureDiagram", "Generates Mermaid diagrams for aggregate structure" do
            command("GenerateStructureDiagram") { attribute :domain_id, String }
          end
        end
      end
    end
  end
end
