HecksPlayground::CLI.handle(:version_tag) do |inv|
  version = inv.args.first
  domain = resolve_domain_option
  next unless domain

  if HecksPlayground::DomainVersioning.exists?(version, base_dir: Dir.pwd)
    say "Version #{version} already exists. Choose a different version label.", :red
    next
  end

  path = HecksPlayground::DomainVersioning.tag(version, domain, base_dir: Dir.pwd)
  say "Tagged #{domain.name} as v#{version}", :green
  say "  Snapshot: #{path}"
end
