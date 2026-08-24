# HecksPlayground::Stats CLI Command
#
# Registers the `hecks_playground stats` command. Loads all domain files in the
# current project and prints comprehensive metrics.
#
#   hecks_playground stats
#   hecks_playground stats --json
#
# stats merged into hecksties

HecksPlayground::CLI.register_command(:stats, "Show comprehensive domain statistics",
  options: {
    json: { type: :boolean, desc: "Output as JSON" }
  }
) do
  require "json" if options[:json]
  project_root = Dir.pwd
  stats = HecksPlayground::Stats::ProjectStats.new(project_root)
  if options[:json]
    puts JSON.pretty_generate(stats.to_h)
  else
    puts stats.summary
  end
end
