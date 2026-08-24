# HecksPlayground::CLI verify command
#
# Runs Bluebook self-verification from the CLI.
#
#   hecks_playground verify                        # progress (dots)
#   hecks_playground verify --format documentation # verbose tree
#
HecksPlayground::CLI.handle(:verify) do |inv|
  bluebook = Dir[File.join(Dir.pwd, "*Bluebook")].first
  Kernel.load(bluebook) if bluebook
  require "hecks_playground/chapters/verify"

  format = (options[:format] || "progress").to_sym
  HecksPlayground::Chapters.verify(format: format)
rescue HecksPlayground::VerificationError => e
  say e.message, :red
  exit 1
end
