# HecksPlayground::Stats
#
# Domain and project statistics: aggregate counts, attribute counts,
# command/event/policy metrics.
#
module HecksPlayground
  # HecksPlayground::Stats
  #
  # Domain and project statistics: aggregate counts, attribute counts, command/event/policy metrics.
  #
  module Stats
    autoload :DomainStats,  "hecks_playground/stats/domain_stats"
    autoload :ProjectStats, "hecks_playground/stats/project_stats"
  end
end
