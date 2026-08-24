# HecksPlayground::CLI -- validate command
#
# The single source of truth for project health. Discovers projects,
# boots them, validates domains, checks UL tag coverage, reports
# capability status. Same output at CLI, server boot, and CI.
#
#   hecks_playground validate                    # validate current directory
#   hecks_playground validate --format json      # machine-readable output
#
HecksPlayground::CLI.handle(:validate) do |inv|
  require "hecks_playground/validate"
  result = HecksPlayground::Validate.run(Dir.pwd, format: options[:format] || "text")
  exit(result[:errors].any? ? 1 : 0)
end
