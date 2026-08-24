# HecksPlayground::CLI — visualize command
#
# Generates Mermaid diagrams from a HecksPlayground domain and outputs them to
# stdout, a file, or a self-contained HTML page in the browser.
#
#   hecks_playground visualize                      # all diagrams to stdout
#   hecks_playground visualize --type structure     # classDiagram only
#   hecks_playground visualize --type behavior      # flowchart only
#   hecks_playground visualize --type flows         # sequenceDiagram only
#   hecks_playground visualize --type slices        # slice flowchart only
#   hecks_playground visualize --type ports         # hexagonal port diagram
#   hecks_playground visualize --browser            # open HTML in browser
#   hecks_playground visualize --output diagram.md  # write to file
#
HecksPlayground::CLI.handle(:visualize) do |inv|
  domain = resolve_domain_option
  unless domain
    say "Error: must be run from a directory containing Bluebook", :red
    next
  end

  diagram_type = (options[:type] || "all").to_sym
  content = build_mermaid(domain, diagram_type)

  if options[:browser]
    open_in_browser(content)
  elsif options[:output]
    File.write(options[:output], content)
    say "Wrote #{options[:output]}", :green
  else
    say content
  end
end

# Build Mermaid markdown for the given domain and diagram type.
#
# @param domain [HecksPlayground::BluebookModel::Structure::Domain]
# @param type   [Symbol] :all, :structure, :behavior, :flows, :slices
# @return [String]
def build_mermaid(domain, type)
  case type
  when :structure
    "```mermaid\n#{HecksPlayground::DomainVisualizer.new(domain).generate_structure}\n```"
  when :behavior
    "```mermaid\n#{HecksPlayground::DomainVisualizer.new(domain).generate_behavior}\n```"
  when :flows
    "```mermaid\n#{HecksPlayground::FlowGenerator.new(domain).generate_mermaid}\n```"
  when :slices
    "```mermaid\n#{HecksPlayground::Features::SliceDiagram.new(domain).generate}\n```"
  when :ports
    "```mermaid\n#{HecksPlayground::DomainVisualizer.new(domain).generate_ports}\n```"
  else
    HecksPlayground::DomainVisualizer.new(domain).generate
  end
end

# Write a self-contained HTML file with Mermaid CDN and open it.
#
# @param mermaid_markdown [String] fenced mermaid markdown blocks
# @return [String] path to temp file
def open_in_browser(mermaid_markdown)
  blocks = mermaid_markdown.scan(/```mermaid\n(.*?)```/m).flatten
  body = blocks.map { |b| "<pre class=\"mermaid\">#{b.strip}</pre>" }.join("\n")
  html = <<~HTML
    <!DOCTYPE html>
    <html>
    <head>
      <meta charset="utf-8">
      <title>HecksPlayground Domain Visualization</title>
    </head>
    <body>
      #{body}
      <script type="module">
        import mermaid from 'https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.esm.min.mjs';
        mermaid.initialize({ startOnLoad: true });
      </script>
    </body>
    </html>
  HTML
  require "tempfile"
  tmp = Tempfile.new(["hecks_playground_visualize", ".html"])
  tmp.write(html)
  tmp.close
  path = tmp.path
  system("open", path)
  say "Opened #{path}", :green
  path
end
