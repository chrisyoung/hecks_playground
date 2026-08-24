# HecksLogging
#
# Structured logging extension for the HecksPlayground command bus. Logs every
# command dispatch to +$stdout+ with the command name, execution duration
# in milliseconds, and optional actor and tenant context. Uses
# +Process::CLOCK_MONOTONIC+ for accurate timing.
#
# Future gem: hecks_playground_logging
#
# Output format:
#   [hecks_playground] CreatePizza 0.3ms actor=admin tenant=acme
#
# Usage:
#   require "hecks_playground_logging"
#   app = HecksPlayground.load(domain)
#   Pizza.create(name: "Margherita")
#   # => [hecks_playground] CreatePizza 0.3ms actor=admin tenant=acme
#
HecksPlayground.describe_extension(:logging,
  description: "Command execution logging",
  adapter_type: :driven,
  config: {},
  wires_to: :command_bus)

HecksPlayground.register_extension(:logging) do |_domain_mod, _domain, runtime|
  # Register command bus middleware that logs each command execution.
  #
  # For each command dispatched:
  # 1. Extracts the unqualified command class name (last segment after "::")
  # 2. Reads the current actor role from HecksPlayground.actor (if set and responds to #role)
  # 3. Reads the current tenant from HecksPlayground.tenant (if set)
  # 4. Records the start time using monotonic clock
  # 5. Calls next_handler to execute the command
  # 6. Computes elapsed time in milliseconds (rounded to 1 decimal)
  # 7. Prints a structured log line to $stdout
  #
  # @param command [Object] the command being dispatched
  # @param next_handler [#call] the next handler in the middleware chain
  # @return [Object] the return value of +next_handler.call+
  runtime.use :logging do |command, next_handler|
    cmd_name = HecksPlayground::Utils.const_short_name(command)
    actor = HecksPlayground.actor&.respond_to?(:role) ? HecksPlayground.actor.role : nil
    tenant = HecksPlayground.tenant
    start = Process.clock_gettime(Process::CLOCK_MONOTONIC)
    result = next_handler.call
    duration = ((Process.clock_gettime(Process::CLOCK_MONOTONIC) - start) * 1000).round(1)
    parts = ["[hecks_playground]", cmd_name, "#{duration}ms"]
    parts << "actor=#{actor}" if actor
    parts << "tenant=#{tenant}" if tenant
    $stdout.puts parts.join(" ")
    result
  end
end
