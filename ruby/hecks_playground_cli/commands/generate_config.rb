HecksPlayground::Chapters.load_aggregates(
  HecksPlayground::Cli::CliTools,
  base_dir: File.expand_path("..", __dir__)
)

HecksPlayground::CLI.handle(:generate_config) do |inv|
  rails_app = lambda { File.exist?("config/application.rb") }

  discover_domains = lambda do
    if options[:domain]
      domain = resolve_domain(options[:domain])
      return domain ? [domain] : nil
    end

    file = find_domain_file
    return [load_domain_file(file)] if file

    domains_dir = File.join(Dir.pwd, "hecks_playground_domains")
    if File.directory?(domains_dir)
      files = Dir[File.join(domains_dir, "*.rb")].sort
      return files.map { |f| load_domain_file(f) } if files.any?
    end

    subdirs = Dir[File.join(Dir.pwd, "*_domain", "*Bluebook")].sort
    return subdirs.map { |f| load_domain_file(f) } if subdirs.any?

    say "No Bluebook, hecks_playground_domains/, or *_domain/ found.", :red
    nil
  end

  adapter_lines = lambda do
    lines = [""]
    lines << "  adapter :memory"
    lines << "  # adapter :sqlite"
    lines << "  # adapter :postgres"
    lines << ""
    lines
  end

  auto_wire_lines = lambda do |meta|
    lines = []
    lines << "  # auto_wire"
    lines << "  # auto_wire except: [:pii]"
    lines << "  # auto_wire only: [:http, :audit]"
    if meta.any?
      lines << ""
      meta.each do |name, m|
        opts = m[:config].map { |k, v| "#{k}: #{v[:default].inspect}" }
        if opts.any?
          lines << "  # extension :#{name}, #{opts.join(", ")}"
        else
          lines << "  # extension :#{name}"
        end
      end
    end
    lines << ""
    lines
  end

  domain_lines_for = lambda do |domain, intro|
    in_deps = intro.listeners[domain.gem_name]
    out_deps = intro.senders[domain.gem_name]
    has_connections = (in_deps && in_deps.any?) || (out_deps && out_deps.any?)

    lines = []
    unless has_connections
      lines << "  domain \"#{domain.gem_name}\""
      next lines
    end

    lines << "  domain \"#{domain.gem_name}\" do"
    if out_deps
      out_deps.each do |target, policies|
        lines << "    sends_to \"#{target}\"  # #{policies.map(&:name).join(", ")}"
      end
    end
    if in_deps
      in_deps.each do |source, policies|
        lines << "    listens_to \"#{source}\"  # #{policies.map(&:name).join(", ")}"
      end
    end
    lines << "  end"
    lines
  end

  build_config = lambda do |domain|
    meta = HecksPlayground.extension_meta
    lines = []
    lines << "HecksPlayground.configure do"
    lines << "  domain \"#{domain.gem_name}\""
    lines.concat(adapter_lines.call)
    lines.concat(auto_wire_lines.call(meta))
    lines << "end"
    lines.join("\n") + "\n"
  end

  build_multi_config = lambda do |domains|
    meta = HecksPlayground.extension_meta
    intro = HecksPlayground::CLI::DomainIntrospector.new(domains)

    lines = []
    lines << "HecksPlayground.configure do"
    domains.each { |d| lines.concat(domain_lines_for.call(d, intro)) }
    lines.concat(adapter_lines.call)
    lines.concat(auto_wire_lines.call(meta))
    lines << "end"
    lines.join("\n") + "\n"
  end

  report_discovery = lambda do |domains|
    say "Found #{domains.size} domain#{"s" if domains.size > 1}:", :green
    domains.each { |d| say "  #{d.name}" }

    if domains.size > 1
      intro = HecksPlayground::CLI::DomainIntrospector.new(domains)
      if intro.listeners.any?
        say ""
        say "Cross-domain connections:", :green
        intro.listeners.each do |listener_gem, sources|
          sources.each do |source_gem, policies|
            policies.each do |pol|
              say "  #{source_gem} -> #{listener_gem} (#{pol.event_name} triggers #{pol.trigger_command})"
            end
          end
        end
      end
    end

    meta = HecksPlayground.extension_meta
    if meta.any?
      say ""
      say "Extensions available:", :green
      meta.each { |name, _| say "  #{name}" }
    end

    say ""
  end

  # Detect extensions
  require "hecks_playground/runtime/load_extensions"
  HecksPlayground::LoadExtensions.require_auto

  domains = discover_domains.call
  next if domains.nil?

  report_discovery.call(domains)
  config = domains.size == 1 ? build_config.call(domains.first) : build_multi_config.call(domains)

  path = rails_app.call ? "config/initializers/hecks_playground.rb" : "app.rb"
  content = path == "app.rb" && !File.exist?(path) ? "require \"hecks_playground\"\n\n#{config}" : config
  write_or_diff(path, content)
  say ""
  say config
end
