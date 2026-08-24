HecksPlayground::CLI.handle(:version) do |inv|
  if options[:domain]
    domain = resolve_domain(options[:domain])
    unless domain
      say "Domain not found: #{options[:domain]}", :red
      next
    end
    spec = ::Gem.loaded_specs[domain.gem_name]
    if spec
      say "#{domain.name}: #{spec.version}"
    else
      dir = File.directory?(options[:domain]) ? options[:domain] : "."
      versioner = HecksPlayground::Versioner.new(dir)
      say "#{domain.name}: #{versioner.current || "not built yet"}"
    end
  else
    say "hecks_playground #{HecksPlayground::VERSION}"
  end
end
