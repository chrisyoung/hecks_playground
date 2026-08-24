# = HecksCli
#
# Entry point for the HecksPlayground command-line interface. Loads the Thor-based
# CLI that provides domain lifecycle commands including:
#
# - +hecks_playground init+ -- Initialize a new HecksPlayground domain project
# - +hecks_playground build+ -- Compile a domain definition into generated Ruby classes
# - +hecks_playground serve+ -- Start an HTTP domain server
# - +hecks_playground console+ -- Open an interactive session with a loaded domain
# - +hecks_playground mcp+ -- Start an MCP (Model Context Protocol) server for AI agents
# - +hecks_playground validate+ -- Lint a domain definition for errors and warnings
# - +hecks_playground dump+ -- Serialize a domain to JSON or YAML
# - +hecks_playground gem+ -- Package a domain as a Ruby gem
#
# == Usage
#
#   require "hecks_playground_cli"
#   HecksPlayground::CLI.start(ARGV)
#
# This is a separate entry point (future gem: +hecks_playground_cli+) to keep the
# CLI dependencies (Thor) isolated from the core framework.
#
# Chapter is loaded lazily when HecksPlayground::CLI is first accessed
# via autoload :CLI, "hecks_playground_cli/cli" in autoloads.rb.
# See hecks_playground_cli/cli.rb for the boot sequence.
