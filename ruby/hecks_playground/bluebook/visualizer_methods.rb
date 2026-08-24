  # HecksPlayground::DomainVisualizerMethods
  #
  # Top-level entry point for Mermaid diagram generation. Extended onto
  # the HecksPlayground module to provide +HecksPlayground.visualize(domain)+ as a convenience
  # method that delegates to DomainVisualizer.
  #
  #   HecksPlayground.visualize(domain)  # => "```mermaid\nclassDiagram\n..."
  #
module HecksPlayground
  module BluebookVisualizerMethods
    # Generate Mermaid diagrams (structure and behavior) for a domain.
    #
    # @param domain [HecksPlayground::BluebookModel::Domain] the domain to visualize
    # @return [String] markdown string with two ```mermaid code blocks
    def visualize(domain)
      DomainVisualizer.new(domain).generate
    end
  end
end
