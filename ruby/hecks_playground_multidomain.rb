# = HecksMultidomain
#
# Multi-domain support for HecksPlayground. Provides filtered event buses,
# cross-domain validation, event directionality, aggregate promotion,
# and cross-domain queries/views.
#
# Lazy-loaded by Boot when hecks_playground_domains/ is detected.
#
#   require "hecks_playground_multidomain"
#
module HecksPlayground
  autoload :FilteredEventBus,  "hecks_playground_multidomain/filtered_event_bus"
  autoload :CrossDomainQuery,  "hecks_playground_multidomain/cross_domain_query"
  autoload :CrossDomainView,   "hecks_playground_multidomain/cross_domain_view"

  # HecksPlayground::MultiDomain
  #
  # Multi-domain support: filtered event buses, cross-domain validation, directionality, and queue wiring.
  #
  module MultiDomain
    autoload :Directionality, "hecks_playground_multidomain/directionality"
    autoload :Validator,      "hecks_playground_multidomain/validator"
    autoload :QueueWiring,    "hecks_playground_multidomain/queue_wiring"
    autoload :Boot,           "hecks_playground_multidomain/boot"
  end
end
