HecksPlayground::Chapters.load_aggregates(
  HecksPlayground::Cli::CliTools,
  base_dir: File.expand_path("..", __dir__)
)

HecksPlayground::CLI.handle(:generate_stub) do |inv|
  type = inv.args[0]
  name = inv.args[1]
  domain = resolve_domain_option
  next unless domain

  type = type.downcase
  result = HecksPlayground::CLI::StubGenerator.new(domain, type, name).generate
  unless result
    say "Unknown type '#{type}'. Use: command, query, aggregate, workflow, service, policy, specification", :red
    next
  end

  result.each do |path, content|
    write_or_diff(path, content)
  end
end
