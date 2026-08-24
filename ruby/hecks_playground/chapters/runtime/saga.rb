# = HecksPlayground::Chapters::Runtime::Saga
#
# Self-describing sub-chapter for sagas and workflows: multi-step
# saga execution with compensation, persistent saga state, and
# branching workflow orchestration.
#
#   HecksPlayground::Chapters::Runtime::Saga.define(builder)
#
module HecksPlayground
  module Chapters
    module Runtime
      # HecksPlayground::Chapters::Runtime::Saga
      #
      # Bluebook sub-chapter for sagas and workflows: saga runner, saga store, and workflow executor.
      #
      module Saga
        def self.define(b)
          b.aggregate "SagaRunner", "Executes multi-step sagas with compensation" do
            command("StartSaga") { attribute :saga_name, String }
          end

          b.aggregate "SagaStore", "Persists saga state across steps" do
            command("SaveState") { attribute :saga_id, String; attribute :state, String }
          end

          b.aggregate "WorkflowExecutor", "Runs multi-step workflows with branching" do
            command("ExecuteWorkflow") { attribute :workflow_name, String }
          end
        end
      end
    end
  end
end
