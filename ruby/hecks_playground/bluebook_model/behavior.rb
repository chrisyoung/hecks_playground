# [antibody-exempt: ruby/hecks_playground/bluebook_model/behavior.rb — kernel-floor
#  behavior namespace of the Ruby grammar mirror ; gains the Factory
#  autoload (first-class factories phase 1). Retires when the Ruby mirror
#  is generated from the grammar bluebooks.]
module HecksPlayground
  module BluebookModel

    # HecksPlayground::BluebookModel::Behavior
    #
    # Namespace for domain behavior representations: commands (intent to change
    # state), events (records of what happened), policies (reactive rules and
    # guards), queries (named lookups), specifications (reusable predicates),
    # workflows (multi-step orchestrations), services (cross-aggregate orchestration),
    # read models (event-driven projections), and event subscribers (arbitrary
    # event handlers).
    #
    # Part of the BluebookModel IR layer. Each child class is a plain data object
    # (intermediate representation) built by the DSL builders and consumed by
    # generators to produce runtime domain code.
    #
    # == Child classes
    #
    #   Behavior::Command         # intent to change aggregate state
    #   Behavior::Condition       # pre/postcondition assertion on a command
    #   Behavior::BluebookEvent     # record that something happened
    #   Behavior::EventSubscriber # arbitrary code block fired on an event
    #   Behavior::Policy          # reactive rule (event->command) or guard block
    #   Behavior::Query           # named, reusable lookup
    #   Behavior::ReadModel       # event-driven denormalized projection
    #   Behavior::Service         # cross-aggregate orchestration
    #   Behavior::Specification   # named, reusable boolean predicate
    #   Behavior::Workflow        # conditional multi-step command orchestration
    #
    module Behavior
      autoload :Command,     "hecks_playground/bluebook_model/behavior/command"
      autoload :Factory,     "hecks_playground/bluebook_model/behavior/factory"
      autoload :Condition,   "hecks_playground/bluebook_model/behavior/condition"
      autoload :BluebookEvent, "hecks_playground/bluebook_model/behavior/bluebook_event"
      autoload :Policy,      "hecks_playground/bluebook_model/behavior/policy"
      autoload :Query,            "hecks_playground/bluebook_model/behavior/query"
      autoload :EventSubscriber, "hecks_playground/bluebook_model/behavior/event_subscriber"
      autoload :Specification,   "hecks_playground/bluebook_model/behavior/specification"
      autoload :Service,         "hecks_playground/bluebook_model/behavior/service"
      autoload :ReadModel,       "hecks_playground/bluebook_model/behavior/read_model"
      autoload :Workflow,        "hecks_playground/bluebook_model/behavior/workflow"
      autoload :CommandStep,    "hecks_playground/bluebook_model/behavior/workflow_step"
      autoload :BranchStep,     "hecks_playground/bluebook_model/behavior/workflow_step"
      autoload :ScheduledStep,  "hecks_playground/bluebook_model/behavior/workflow_step"
      autoload :Saga,           "hecks_playground/bluebook_model/behavior/saga"
      autoload :SagaStep,       "hecks_playground/bluebook_model/behavior/saga_step"
      autoload :ProcessManager, "hecks_playground/bluebook_model/behavior/process_manager"
      autoload :Cadence,        "hecks_playground/bluebook_model/behavior/cadence"
      autoload :BlockGrammar,   "hecks_playground/bluebook_model/behavior/block_grammar"
      autoload :Given,          "hecks_playground/bluebook_model/behavior/given"
      autoload :Mutation,       "hecks_playground/bluebook_model/behavior/mutation"
    end
  end
end
