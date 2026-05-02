# spec/hecks/runtime/llm_dispatcher_spec.rb
#
# Contract for Hecks::Runtime::LlmDispatcher i23 §4 pipeline. Step 6 owns
# the Spend + CircuitBreaker gating + post-call bookkeeping branches:
#
#   - breaker open      → Skipped(reason: "breaker_open")
#   - budget exceeded   → Skipped(reason: "budget_exceeded")
#   - override bypass   → call proceeds, LlmInvocationOverbudgetBypass logged
#   - provider success  → RecordCall + RecordSuccess + LlmInvocationCompleted
#   - provider failure  → RecordFailure + LlmInvocationFailed + raise
#   - partial-record    → response still returned, error logged
#
$LOAD_PATH.unshift File.expand_path("../../../../ruby", __dir__)
require "hecks"
require "hecksagon/structure/llm_adapter"
require "hecks/runtime/llm_dispatcher"
require_relative "llm_dispatcher_doubles"

RSpec.describe Hecks::Runtime::LlmDispatcher do
  FakeLogger   = LlmDispatcherDoubles::FakeLogger
  FakeSpend    = LlmDispatcherDoubles::FakeSpend
  FakeBreaker  = LlmDispatcherDoubles::FakeBreaker
  FakeProvider = LlmDispatcherDoubles::FakeProvider

  let(:adapter) do
    Hecksagon::Structure::LlmAdapter.new(
      name: :speech,
      prompt_template: "Say {{idea}}",
      model: "claude-sonnet-4",
      max_tokens: 200,
      response_into_target: "Curator.CurateMusing",
      response_into_attr: :response,
      backend: :claude
    )
  end

  let(:logger)   { FakeLogger.new }
  let(:spend)    { FakeSpend.new }
  let(:breaker)  { FakeBreaker.new }
  let(:provider) do
    FakeProvider.new(returns: { body: "haiku", tokens_in: 12, tokens_out: 5,
                                cost_usd: 0.0001, provider: "claude" })
  end

  let(:fixed_now) { Time.utc(2026, 5, 2, 12, 0, 0) }

  def dispatch(**overrides)
    described_class.call(
      adapter: adapter,
      attrs: { idea: "spring rain" },
      provider: provider,
      spend: spend,
      breaker: breaker,
      logger: logger,
      now: fixed_now,
      **overrides
    )
  end

  # ----------------------------------------------------------------------
  # Pipeline
  # ----------------------------------------------------------------------

  describe "breaker gate (step 3)" do
    let(:breaker) { FakeBreaker.new(open: true) }

    it "returns LlmInvocationSkipped(reason: breaker_open) without invoking the provider" do
      result = dispatch
      expect(result).to be_a(described_class::Skipped)
      expect(result.reason).to eq("breaker_open")
      expect(provider.calls).to be_empty
      expect(logger.event_names).to include("LlmInvocationSkipped")
      skipped_event = logger.events.find { |e| e[:event] == "LlmInvocationSkipped" }
      expect(skipped_event[:reason]).to eq("breaker_open")
      expect(skipped_event[:kind]).to eq("speech_api")
    end

    it "does NOT call Spend.RecordCall when the breaker short-circuits" do
      dispatch
      expect(spend.recorded_calls).to be_empty
    end

    it "does NOT call CircuitBreaker.RecordSuccess when the breaker short-circuits" do
      dispatch
      expect(breaker.successes).to be_empty
    end
  end

  describe "spend gate (step 4)" do
    let(:spend) { FakeSpend.new(over_budget: true) }

    it "returns LlmInvocationSkipped(reason: budget_exceeded) without invoking the provider" do
      result = dispatch(period: "2026-05-02")
      expect(result).to be_a(described_class::Skipped)
      expect(result.reason).to eq("budget_exceeded")
      expect(provider.calls).to be_empty
      skipped_event = logger.events.find { |e| e[:event] == "LlmInvocationSkipped" }
      expect(skipped_event[:reason]).to eq("budget_exceeded")
      expect(skipped_event[:period]).to eq("2026-05-02")
    end

    it "is bypassed when override_active? returns true ; logs LlmInvocationOverbudgetBypass" do
      bypass_spend = FakeSpend.new(over_budget: true, override: true)
      result = described_class.call(
        adapter: adapter, attrs: {}, provider: provider, spend: bypass_spend,
        breaker: breaker, logger: logger, now: fixed_now
      )
      expect(result).to be_a(described_class::Response)
      expect(provider.calls.size).to eq(1)
      expect(logger.event_names).to include("LlmInvocationOverbudgetBypass")
      expect(logger.event_names).to include("LlmInvocationCompleted")
      expect(logger.event_names).not_to include("LlmInvocationSkipped")
    end
  end

  describe "provider success (step 6)" do
    it "calls Spend.RecordCall with the provider response telemetry" do
      dispatch
      expect(spend.recorded_calls.size).to eq(1)
      call = spend.recorded_calls.first
      expect(call[:provider]).to eq("claude")
      expect(call[:tokens_in]).to eq(12)
      expect(call[:tokens_out]).to eq(5)
      expect(call[:cost_usd]).to be_within(0.000001).of(0.0001)
      expect(call[:command_ref]).to eq("Curator.CurateMusing")
      expect(call[:recorded_at]).to eq(fixed_now.iso8601)
    end

    it "calls CircuitBreaker.RecordSuccess with the resolved breaker_kind" do
      dispatch
      expect(breaker.successes).to eq(["speech_api"])
    end

    it "emits LlmInvocationCompleted on the logger after the bookkeeping commands" do
      dispatch
      expect(logger.event_names).to include("LlmInvocationCompleted")
      completed = logger.events.find { |e| e[:event] == "LlmInvocationCompleted" }
      expect(completed[:adapter]).to eq(:speech)
      expect(completed[:tokens_in]).to eq(12)
      expect(completed[:tokens_out]).to eq(5)
    end

    it "returns a Response wrapping the provider body + telemetry" do
      result = dispatch
      expect(result).to be_a(described_class::Response)
      expect(result.body).to eq("haiku")
      expect(result.completed?).to eq(true)
      expect(result.skipped?).to eq(false)
    end

    it "honors an explicit breaker_kind override" do
      dispatch(breaker_kind: :overridden_kind)
      expect(breaker.successes).to eq(["overridden_kind"])
    end
  end

  describe "provider failure (step 6)" do
    let(:provider) { FakeProvider.new(raises: RuntimeError.new("boom")) }

    it "calls CircuitBreaker.RecordFailure" do
      expect { dispatch }.to raise_error(RuntimeError, /boom/)
      expect(breaker.failures).to eq(["speech_api"])
    end

    it "does NOT call Spend.RecordCall (failed call has no telemetry to record)" do
      expect { dispatch }.to raise_error(RuntimeError)
      expect(spend.recorded_calls).to be_empty
    end

    it "emits LlmInvocationFailed before raising" do
      expect { dispatch }.to raise_error(RuntimeError)
      failed = logger.events.find { |e| e[:event] == "LlmInvocationFailed" }
      expect(failed).not_to be_nil
      expect(failed[:adapter]).to eq(:speech)
      expect(failed[:error_message]).to include("boom")
    end

    it "does NOT emit LlmInvocationCompleted" do
      expect { dispatch }.to raise_error(RuntimeError)
      expect(logger.event_names).not_to include("LlmInvocationCompleted")
    end
  end

  describe "partial bookkeeping failure (crash ordering — i23 §4)" do
    # Provider succeeds but Spend.RecordCall crashes. The dispatcher must
    # still emit LlmInvocationCompleted and return the response. Missing
    # spend record is preferable to a lost provider response — the
    # reconcile daemon picks up the gap.
    let(:spend) do
      FakeSpend.new(raise_on_record: RuntimeError.new("db unavailable"))
    end

    it "still returns the wrapped Response" do
      result = dispatch
      expect(result).to be_a(described_class::Response)
      expect(result.body).to eq("haiku")
    end

    it "still emits LlmInvocationCompleted" do
      dispatch
      expect(logger.event_names).to include("LlmInvocationCompleted")
    end

    it "logs spend_record_call_failed with the underlying error" do
      dispatch
      err_event = logger.events.find { |e| e[:event] == "spend_record_call_failed" }
      expect(err_event).not_to be_nil
      expect(err_event[:error_message]).to include("db unavailable")
    end

    it "still calls CircuitBreaker.RecordSuccess (the provider call did succeed)" do
      dispatch
      expect(breaker.successes).to eq(["speech_api"])
    end
  end

  describe "default breaker_kind" do
    it "defaults to '#{"<adapter_name>"}_api'" do
      adapter2 = Hecksagon::Structure::LlmAdapter.new(name: :curate)
      described_class.call(
        adapter: adapter2, attrs: {}, provider: provider,
        spend: spend, breaker: breaker, logger: logger, now: fixed_now
      )
      expect(breaker.successes).to eq(["curate_api"])
    end
  end

  describe "without spend / breaker (gating disabled)" do
    it "passes through to the provider when both ports are nil" do
      result = described_class.call(
        adapter: adapter, attrs: {}, provider: provider,
        spend: nil, breaker: nil, logger: logger, now: fixed_now
      )
      expect(result).to be_a(described_class::Response)
      expect(provider.calls.size).to eq(1)
    end
  end
end
