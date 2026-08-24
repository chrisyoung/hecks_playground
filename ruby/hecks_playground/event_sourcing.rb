# HecksPlayground::EventSourcing
#
# Top-level module for all event-sourcing concerns: optimistic concurrency,
# CQRS read model stores, event upcasting, projections, outbox, process
# managers, snapshots, and time travel. Each sub-module is autoloaded.
#
# == Usage
#
#   require "hecks_playground/event_sourcing"
#   HecksPlayground::EventSourcing::Concurrency.stamp!(agg, 1)
#   HecksPlayground::EventSourcing::EventStore.new.append("Pizza-1", event)
#
module HecksPlayground
  # HecksPlayground::EventSourcing
  #
  # Top-level module for event-sourcing concerns: concurrency, read models, snapshots, outbox, and process managers.
  #
  module EventSourcing
    autoload :Concurrency,       "hecks_playground/event_sourcing/concurrency"
    autoload :VersionCheckStep,  "hecks_playground/event_sourcing/version_check_step"
    autoload :ReadModelStore,    "hecks_playground/event_sourcing/read_model_store"
    autoload :EventStore,        "hecks_playground/event_sourcing/event_store"
    autoload :UpcasterRegistry,  "hecks_playground/event_sourcing/upcaster_registry"
    autoload :UpcasterEngine,    "hecks_playground/event_sourcing/upcaster_engine"
    autoload :ProjectionRebuilder, "hecks_playground/event_sourcing/projection_rebuilder"
    autoload :Outbox,            "hecks_playground/event_sourcing/outbox"
    autoload :OutboxStep,        "hecks_playground/event_sourcing/outbox_step"
    autoload :OutboxPoller,      "hecks_playground/event_sourcing/outbox_poller"
    autoload :ProcessManager,    "hecks_playground/event_sourcing/process_manager"
    autoload :SnapshotStore,     "hecks_playground/event_sourcing/snapshot_store"
    autoload :Reconstitution,    "hecks_playground/event_sourcing/reconstitution"
    autoload :TimeTravel,        "hecks_playground/event_sourcing/time_travel"
  end
end
