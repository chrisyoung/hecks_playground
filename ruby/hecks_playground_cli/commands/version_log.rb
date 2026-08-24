HecksPlayground::Chapters.load_aggregates(
  HecksPlayground::Cli::CliInternals,
  base_dir: __dir__
)

HecksPlayground::CLI.handle(:version_log) do |inv|
  entries = HecksPlayground::DomainVersioning.log(base_dir: Dir.pwd)

  if entries.empty?
    say "No versions tagged yet. Run `hecks_playground version_tag <version>` to tag one.", :yellow
    next
  end

  lines = HecksPlayground::CLI::VersionLogFormatter.format(entries)
  lines.each { |line| say line }
end
