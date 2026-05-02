module Hecks
  class Runtime

    # Hecks::Runtime::LlmDispatcher
    #
    # Stub-only placeholder for the i23 Stage A step-8 boot wiring.
    # The real dispatcher lands in step 4 (PR #561) — and step 6 (PR
    # #564) extends it with spend/circuit-breaker gates. This file only
    # exists so that `require "hecks/runtime/llm_dispatcher"` resolves
    # in `ruby/hecks/runtime.rb` and `Runtime#llm` can name a callable
    # symbol while sibling PRs are still in flight.
    #
    # When step 4 (or step 6, whichever lands second) merges into the
    # `llm-adapter` base branch, this stub MUST be deleted in favour of
    # the real implementation. The conflict here is the merge signal.
    #
    #   # Step 4 will replace this with a substituting + provider-routing
    #   # implementation. For now :
    #   Hecks::Runtime::LlmDispatcher.call(adapter, attrs)
    #   # => raises NotImplementedError
    #
    module LlmDispatcher
      module_function

      # Stub call — siblings step 4 (PR #561) and step 6 (PR #564)
      # ship the real implementation. Step 8 only needs the symbol
      # resolvable so Runtime#llm and the runtime spec can reference it.
      #
      # @param _adapter [Hecksagon::Structure::LlmAdapter]
      # @param _attrs [Hash]
      # @raise [NotImplementedError] always — siblings own the real call
      def call(_adapter, _attrs = {})
        raise NotImplementedError,
              "Hecks::Runtime::LlmDispatcher.call : step 4 (PR #561) " \
              "lands the real implementation ; this stub exists only " \
              "so step 8 boot wiring can name the dispatcher symbol."
      end
    end
  end
end
