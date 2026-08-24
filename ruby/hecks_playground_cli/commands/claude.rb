# HecksPlayground::CLI :claude command
# Starts file watchers and launches Claude Code in one step.
# Usage: hecks_playground claude [ARGS...]
HecksPlayground::CLI.handle(:claude) do |inv|
  script = ::Gem.bin_path("hecks_playground", "hecks_playground_claude")
  exec script, *inv.args
rescue ::Gem::Exception
  say "hecks_playground_claude not found. Is the hecks_playground gem installed?", :red
end
