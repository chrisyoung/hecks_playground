HecksPlayground::CLI.handle(:diff) do |inv|
  # Resolve the two domains to diff
  if options[:v1] && options[:v2]
    old_domain = HecksPlayground::DomainVersioning.load_version(options[:v1], base_dir: Dir.pwd)
    new_domain = HecksPlayground::DomainVersioning.load_version(options[:v2], base_dir: Dir.pwd)
    unless old_domain
      say "Version #{options[:v1]} not found in db/hecks_playground_versions/", :red
      next
    end
    unless new_domain
      say "Version #{options[:v2]} not found in db/hecks_playground_versions/", :red
      next
    end
    label = "v#{options[:v1]} -> v#{options[:v2]}"
  elsif options[:v1]
    old_domain = HecksPlayground::DomainVersioning.load_version(options[:v1], base_dir: Dir.pwd)
    unless old_domain
      say "Version #{options[:v1]} not found in db/hecks_playground_versions/", :red
      next
    end
    new_domain = resolve_domain_option
    next unless new_domain
    label = "v#{options[:v1]} -> working"
  else
    new_domain = resolve_domain_option
    next unless new_domain

    latest = HecksPlayground::DomainVersioning.latest_version(base_dir: Dir.pwd)
    if latest
      old_domain = HecksPlayground::DomainVersioning.load_version(latest, base_dir: Dir.pwd)
      label = "v#{latest} -> working"
    else
      snapshot_path = HecksPlayground::Migrations::DomainSnapshot::DEFAULT_PATH
      unless HecksPlayground::Migrations::DomainSnapshot.exists?(path: snapshot_path)
        say "No snapshot or tagged version found. Run `hecks_playground version_tag` or `hecks_playground build` first.", :yellow
        next
      end
      old_domain = HecksPlayground::Migrations::DomainSnapshot.load(path: snapshot_path)
      label = "snapshot -> working"
    end
  end

  changes = HecksPlayground::Migrations::DomainDiff.call(old_domain, new_domain)

  if changes.empty?
    say "No changes detected (#{label}).", :green
    next
  end

  classified = HecksPlayground::DomainVersioning::BreakingClassifier.classify(changes)

  say "#{changes.size} change#{"s" if changes.size != 1} (#{label}):", :yellow
  say ""

  classified.each do |entry|
    suffix = entry[:breaking] ? "  <- BREAKING" : ""
    color = entry[:breaking] ? :red : :green
    say "  #{entry[:label]}#{suffix}", color
  end

  breaking = classified.count { |e| e[:breaking] }
  if breaking > 0
    say ""
    say "#{breaking} breaking change#{"s" if breaking != 1}!", :red
  end
end
