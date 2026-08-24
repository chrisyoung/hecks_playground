# = HecksPlayground::VERSION
#
# Defines the current version of the HecksPlayground gem itself. This is the
# *framework* version, separate from individual domain versions managed
# by {HecksPlayground::Versioner} (which uses CalVer: YYYY.MM.DD.N).
#
# Used by the gemspec for gem packaging and by the CLI for +hecks_playground --version+
# output.
#
# @example
#   HecksPlayground::VERSION  # => "0.1.0"
#
module HecksPlayground
  VERSION = "2026.04.26.4"
end
