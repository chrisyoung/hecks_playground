# frozen_string_literal: true

module Hecks
  class Runtime

    # Hecks::Runtime::LlmDispatcher
    #
    # Wraps an LlmAdapter invocation in the i23 §4 pipeline:
    #
    #   1. resolve provider                 (step 4 — minimal stub here)
    #   2. resolve breaker_kind             ("#{adapter.name}_api" default)
    #   3. CircuitBreaker.IsOpen?           → Skipped(reason: "breaker_open")
    #   4. Spend.IsOverBudget?               → Skipped(reason: "budget_exceeded")
    #      (skipped when override_active == "yes" + override_until >= now ;
    #       logs LlmInvocationOverbudgetBypass — see TODO below if override
    #       lookup is unavailable in this runtime)
    #   5. PromptScaffolder                 (step 7 — pass-through here)
    #   6. provider.invoke(adapter, attrs)
    #      ├─ success: Spend.RecordCall + CircuitBreaker.RecordSuccess
    #      │           + LlmInvocationCompleted
    #      └─ failure: CircuitBreaker.RecordFailure + LlmInvocationFailed
    #                  → raise
    #
    # Crash ordering (i23 §4): provider success + RecordCall failure
    # still emits LlmInvocationCompleted and returns the response.
    # Missing-record beats lost-response — the reconcile daemon catches up.
    #
    #   result = LlmDispatcher.call(
    #     adapter:  adapter,
    #     attrs:    { idea: "concise haiku" },
    #     provider: provider,            # responds to .invoke(adapter, attrs) → Response
    #     spend:    spend_port,          # responds to .over_budget?, .record_call, .override_active?
    #     breaker:  breaker_port,        # responds to .open?, .record_success, .record_failure
    #     logger:   logger               # optional ; receives event hashes via #<<
    #   )
    #   #=> Response | Skipped     (raises LlmAdapterError on provider failure)
    #
    module LlmDispatcher
      # Returned in place of a Response when the dispatcher gates the call
      # before the provider even runs (breaker open, budget exceeded, or a
      # provider-availability skip emitted by step 4 / 5). Carries the
      # reason so callers can branch (e.g. fall back to a cached response).
      Skipped = Struct.new(:adapter_name, :reason, :details, keyword_init: true) do
        def skipped? = true
        def completed? = false
      end

      # Returned in place of a Response when the provider invocation
      # succeeded. Provider responses are wrapped so callers can rely on
      # the Response/Skipped duality without sniffing types.
      Response = Struct.new(:adapter_name, :body, :tokens_in, :tokens_out,
                            :cost_usd, :provider, keyword_init: true) do
        def skipped? = false
        def completed? = true
      end

      module_function

      # Dispatch +adapter+ through the gated pipeline.
      #
      # @param adapter [Hecksagon::Structure::LlmAdapter]
      # @param attrs [Hash{Symbol=>Object}]
      # @param provider [#invoke] receives (adapter, attrs) ; returns hash-like
      # @param spend [#over_budget?, #record_call] (optional — gating disabled when nil)
      # @param breaker [#open?, #record_success, #record_failure] (optional)
      # @param breaker_kind [String, Symbol, nil] override "#{name}_api" default
      # @param period [String] spend bucket key (e.g. "2026-05-02")
      # @param logger [#<<, nil] event sink ; receives Hash entries
      # @param now [Time, nil] injectable clock for override-bypass (Time.now default)
      # @return [Response, Skipped]
      # @raise [Hecks::LlmAdapterError] when the provider invocation raises
      def call(adapter:, attrs: {}, provider:, spend: nil, breaker: nil,
               breaker_kind: nil, period: nil, logger: nil, now: nil)
        kind = (breaker_kind || "#{adapter.name}_api").to_s
        clock = now || Time.now
        bucket = period || default_period(clock)

        # Step 3 — circuit breaker open?
        if breaker && safe_call(breaker, :open?, kind: kind, default: false)
          return emit_skipped(adapter, "breaker_open", { kind: kind }, logger)
        end

        # Step 4 — spend over budget? (with override bypass)
        if spend
          bypass = override_bypass?(spend, bucket, clock)
          if bypass
            log(logger, event: "LlmInvocationOverbudgetBypass",
                       adapter: adapter.name, period: bucket)
          elsif safe_call(spend, :over_budget?, period: bucket, default: false)
            return emit_skipped(adapter, "budget_exceeded", { period: bucket }, logger)
          end
        end

        # Step 5 — PromptScaffolder slot (step 7 will plug in here).
        # For step 6, attrs flows straight to the provider unchanged.

        # Step 6 — provider invoke
        invoke_provider(adapter, attrs, kind, bucket, clock,
                        provider: provider, spend: spend, breaker: breaker,
                        logger: logger)
      end

      # ------------------------------------------------------------------
      # internals
      # ------------------------------------------------------------------

      # Run provider.invoke and the success / failure post-processing.
      # Crash-ordered: if RecordCall fails after a successful provider
      # response, we still emit LlmInvocationCompleted and return the
      # response — missing-record beats lost-response.
      def invoke_provider(adapter, attrs, kind, period, clock,
                          provider:, spend:, breaker:, logger:)
        begin
          response = provider.invoke(adapter, attrs)
        rescue => e
          if breaker
            safe_invoke(breaker, :record_failure, { kind: kind }, logger,
                        on_error: "breaker_record_failure_failed")
          end
          log(logger, event: "LlmInvocationFailed",
                     adapter: adapter.name, error_class: e.class.name,
                     error_message: e.message)
          raise build_failure_error(adapter, e)
        end

        wrapped = wrap_response(adapter, response, provider)

        if spend
          safe_invoke(spend, :record_call, {
            provider: wrapped.provider.to_s,
            tokens_in: wrapped.tokens_in.to_i,
            tokens_out: wrapped.tokens_out.to_i,
            cost_usd: wrapped.cost_usd.to_f,
            command_ref: adapter.response_into_target.to_s,
            recorded_at: clock.iso8601
          }, logger, on_error: "spend_record_call_failed")
        end

        if breaker
          safe_invoke(breaker, :record_success, { kind: kind }, logger,
                      on_error: "breaker_record_success_failed")
        end

        log(logger, event: "LlmInvocationCompleted",
                   adapter: adapter.name, period: period,
                   tokens_in: wrapped.tokens_in, tokens_out: wrapped.tokens_out)
        wrapped
      end

      # Normalize a provider's return value into a Response struct. Accepts:
      #   - Response (passed through)
      #   - Hash with :body / :tokens_in / :tokens_out / :cost_usd / :provider keys
      #   - String (treated as body, zero token / cost telemetry)
      def wrap_response(adapter, raw, provider)
        return raw if raw.is_a?(Response)
        if raw.is_a?(Hash)
          Response.new(
            adapter_name: adapter.name,
            body: raw[:body] || raw["body"] || "",
            tokens_in: (raw[:tokens_in] || raw["tokens_in"] || 0).to_i,
            tokens_out: (raw[:tokens_out] || raw["tokens_out"] || 0).to_i,
            cost_usd: (raw[:cost_usd] || raw["cost_usd"] || 0.0).to_f,
            provider: raw[:provider] || raw["provider"] || provider_name(provider, adapter)
          )
        else
          Response.new(
            adapter_name: adapter.name,
            body: raw.to_s, tokens_in: 0, tokens_out: 0, cost_usd: 0.0,
            provider: provider_name(provider, adapter)
          )
        end
      end

      def provider_name(provider, adapter)
        return provider.name if provider.respond_to?(:name)
        adapter.backend.to_s
      end

      # Override bypass — gated on whichever spend port the runtime wires.
      # Falls back to "no bypass" when the port doesn't expose
      # #override_active?, so dispatchers wired to a thinner port stay
      # safe-by-default. Emits a ship-without-bypass TODO trail when the
      # port is missing the hook. See PR body — Override aggregate lookup
      # gap is documented there.
      def override_bypass?(spend, period, clock)
        return false unless spend.respond_to?(:override_active?)
        safe_call(spend, :override_active?, period: period, now: clock, default: false)
      end

      # Emit a Skipped + log the event ; helper to keep .call concise.
      def emit_skipped(adapter, reason, details, logger)
        log(logger, event: "LlmInvocationSkipped",
                   adapter: adapter.name, reason: reason, **details)
        Skipped.new(adapter_name: adapter.name, reason: reason, details: details)
      end

      # safe_call — call a port method that takes only kwargs ; returns
      # default on any raise. Used for IsOpen? / IsOverBudget? — a gating
      # query failure must NOT block the call (graceful degradation).
      def safe_call(port, method, default:, **kwargs)
        return default unless port.respond_to?(method)
        port.public_send(method, **kwargs)
      rescue
        default
      end

      # safe_invoke — call a port command with kwargs ; logs on raise but
      # never re-raises. Used for RecordCall / RecordSuccess / RecordFailure
      # because the dispatcher's crash-ordering rule says these are
      # secondary to the provider response.
      def safe_invoke(port, method, kwargs, logger, on_error:)
        return false unless port.respond_to?(method)
        port.public_send(method, **kwargs)
        true
      rescue => e
        log(logger, event: on_error, error_class: e.class.name,
                   error_message: e.message)
        false
      end

      def log(logger, payload)
        return unless logger
        logger << payload if logger.respond_to?(:<<)
      end

      # Wrap a provider exception in a uniform LlmAdapterError so callers
      # have one rescue type. Defers definition of the error class to the
      # errors file (loaded with the Hecks bootstrap) when present —
      # otherwise falls back to a plain RuntimeError preserving the cause.
      def build_failure_error(adapter, cause)
        if defined?(Hecks::LlmAdapterError)
          err = Hecks::LlmAdapterError.new(
            "llm adapter :#{adapter.name} failed: #{cause.message}",
            adapter: adapter.name, cause: cause
          )
          err.set_backtrace(cause.backtrace) if cause.backtrace
          err
        else
          err = RuntimeError.new("llm adapter :#{adapter.name} failed: #{cause.message}")
          err.set_backtrace(cause.backtrace) if cause.backtrace
          err
        end
      end

      # Default spend bucket key — UTC date string. Period granularity
      # (hour/day/month) is a runtime knob ; ":day" matches the budget
      # bluebook's default rollup.
      def default_period(clock)
        clock.utc.strftime("%Y-%m-%d")
      end
    end
  end
end
