HecksPlayground::CLI.register_command(:mcp, "Start MCP server — build domains (default) or serve one (--domain)",
  options: {
    domain:  { type: :string, desc: "Domain gem name or path (serves it as MCP tools)" },
    version: { type: :string, desc: "Domain version" }
  }
) do
  if options[:domain]
    domain = resolve_domain(options[:domain])
    unless domain
      say "Domain not found: #{options[:domain]}", :red
      next
    end
    require "hecks_playground_ai"
    HecksPlayground::MCP::DomainServer.new(domain).run
  else
    require "hecks_playground_ai"
    HecksPlayground::McpServer.new.run
  end
end
