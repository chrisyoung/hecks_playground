# HecksagainRuntime::BehaviorsRunner::Expectations
#
# One test case, start to finish: boot the right scope, replay `setup`
# dispatches, dispatch (or query) the command under test, check `expect`.
# Split out of behaviors_runner.rb to keep both files under the 200-LOC
# rule -- this is the half that touches a live runtime per test; the
# parent file is discovery + aggregation.
#
# A `setup`'s aggregate is not declared by the grammar (rust/src/
# behaviors_runner.rs passes `setup.command` straight to dispatch AS the
# fqn -- it resolves only because Rust's Domain can disambiguate a
# bareword command within one small file ; hecks's dispatcher requires the
# full "Domain::Aggregate.Command" form). `Dispatching.qualify`
# (dispatching.rb) does that resolution, along with choosing the directory
# a test boots against and wrapping a dispatch in #335's routing envelope.
require_relative "dispatching"

module HecksagainRuntime
  module BehaviorsRunner
    module Expectations
      module_function

      REFUSAL_CLASSES = Hecks::Runtime::DOMAIN_REFUSALS
      NON_STATE_KEYS  = %i[ok emits refused count].freeze

      def run_one(test, source:, corpus_root: nil, staged_root: nil)
        return pass_result(test, pending: true) if test.pending?

        dir = nil
        if test.cross_cascade?
          return error_result(test, "cross_cascade test but no corpus_root given") unless corpus_root
          dir = staged_root || HecksagainRuntime.stage_flat_corpus(corpus_root)
        else
          # `:cascade`/`:driving_tick` (slice 1.3, driving-adapter-grammar
          # port) -- narrow, scoped widening of the isolated-dir case, NOT
          # a switch to the full staged corpus. A `:cascade` test proves a
          # cross-FILE fan-out (tools.behaviors's own "ShellAdapter +
          # Sprint14SmokeFanout" -- two SEPARATE `.hecksagon` files, this
          # source bluebook's own sibling `hecksagons/` folder), and
          # `:driving_tick` proves a `driving on cron/interval` handler
          # actually fires -- neither exists without that folder's
          # content also present in the isolated dir. Deliberately NOT
          # applied to every kind: agent_tool.hecksagon (`driven on
          # Tools::AgentTool.AgentMessageSent`) lives in the SAME sibling
          # folder, and the three plain `SendMessage` tests
          # (`expect emits: ["AgentMessageSent"]`, exact) would gain an
          # unwanted extra cascade if it loaded for them too.
          include_hecksagons = %i[cascade driving_tick].include?(test.kind)
          dir = Dispatching.isolated_dir_for(source, include_hecksagons: include_hecksagons)
        end

        runtime    = Hecks.boot(dir, install_facade: false)
        domain_name = runtime.registry.bluebooks.keys.first
        return error_result(test, "could not determine a domain name from #{source}") unless domain_name

        failed_setup = replay_setups(test, runtime, domain_name)
        return failed_setup if failed_setup

        return run_driving_tick(test, runtime) if test.driving_tick?

        test.query? ? run_query(test, runtime, domain_name) : run_command(test, runtime, domain_name)
      rescue *REFUSAL_CLASSES => e
        check_refusal(test, e)
      rescue StandardError => e
        error_result(test, "#{e.class}: #{e.message}")
      ensure
        FileUtils.remove_entry(dir) if dir && !test.cross_cascade? && File.exist?(dir)
      end

      # Setups get their OWN refusal scope, separate from the command
      # under test. Sharing one `rescue *REFUSAL_CLASSES` (what this file
      # used to do) lets a BROKEN SETUP spuriously satisfy `expect
      # refused: "..."` whenever its own refusal text happens to contain
      # the expected description -- a green check for a test that never
      # reached the situation it claims to describe. A refusal during
      # `setup` is therefore unconditionally an error. Lesson taken from
      # the published gem's own Hecks::Behaviors::Expectations, which
      # calls this deviation out explicitly in its header.
      def replay_setups(test, runtime, domain_name)
        bluebook = runtime.registry.bluebook(domain_name)
        current  = nil
        test.setups.each do |setup|
          current = setup
          verb = Dispatching.qualify(setup.command, test.on_aggregate, domain_name, bluebook)
          Dispatching.dispatch_command(runtime, verb, setup.args)
        end
        nil
      rescue *REFUSAL_CLASSES => e
        error_result(test, "setup #{current&.command.inspect} refused: #{e.message}")
      end

      def run_command(test, runtime, domain_name)
        result = Dispatching.dispatch_command(runtime, test.fqn(domain_name), test.input)

        return fail_result(test, "expected refused: #{test.expect[:refused].inspect} but dispatch succeeded") if test.expect.key?(:refused)

        if (expected_emits = test.expect[:emits])
          actual = result.events.map(&:name)
          return fail_result(test, "expected emits: #{expected_emits.inspect}, got #{actual.inspect}") unless actual == expected_emits
        end

        state = result.state || {}
        test.expect.each do |key, expected|
          next if NON_STATE_KEYS.include?(key)

          raw    = state.key?(key) ? state[key] : state[key.to_s]
          actual = normalize(raw)
          exp    = normalize(expected)
          return fail_result(test, "expected #{key}: #{expected.inspect}, got #{actual.inspect}") unless actual == exp
        end

        pass_result(test)
      end

      # `kind: :driving_tick` (slice 1.3) was written to prove a `driving
      # on cron/interval` handler dispatches deterministically, by firing
      # every cron handler once through `Hecks::Runtime::DrivingScheduler
      # #fire_all!`. THAT CLASS DOES NOT EXIST AND NEVER DID -- not in
      # hecks 1.3.0, and not in the vendored copy this runner was written
      # against either (checked with `git log -S` over vendor/). The old
      # code called it anyway, so the single `:driving_tick` test in the
      # corpus has always died on a NameError that the catch-all rescue
      # relabelled. Reported as its own explicit error rather than
      # silently passed or quietly skipped: the capability is genuinely
      # missing from the gem, and a runner that pretends otherwise is the
      # exact failure this file's fix exists to end.
      def run_driving_tick(test, _runtime)
        error_result(test,
                     "kind: :driving_tick needs a driving-adapter scheduler (fire every `driving on cron` " \
                     "handler once); hecks #{Hecks::VERSION} ships none -- there is no " \
                     "Hecks::Runtime::DrivingScheduler or any other cron-firing API to port onto")
      end

      def run_query(test, runtime, domain_name)
        rows  = runtime.query(test.fqn(domain_name), **test.input)
        count = rows.is_a?(Array) ? rows.size : (rows.nil? ? 0 : 1)
        expected = test.expect[:count]
        return fail_result(test, "expected count: #{expected}, got #{count}") if expected && count != expected

        pass_result(test)
      end

      def check_refusal(test, error)
        expected = test.expect[:refused]
        return error_result(test, "unexpected refusal (#{error.class}): #{error.message}") unless expected

        msg = error.message.to_s
        # hecksagain's given/ensures refusals render "Command refused -- description"
        # (runtime/command_rules/admissibility.rb) ; behaviors files expect the bare
        # description (Rust's own convention) -- end_with?/include? bridges the prefix.
        return pass_result(test) if msg == expected || msg.end_with?(expected) || msg.include?(expected)

        fail_result(test, "expected refused: #{expected.inspect}, got #{msg.inspect}")
      end

      # `on_aggregate` is the right guess for MOST setups (the corpus's own
      # convention is a chain of same-aggregate lifecycle steps before the
      # command under test). It is the WRONG guess for a genuine
      # cross-aggregate cascade test -- `tests "Add", on: "Task"` seeding a
      # `setup "Capture"` that actually creates the owning Story, found live
      # in plan.behaviors's own "Task.Add cascades through BumpOnTaskAdded"
      # test. So: try `on_aggregate` first (cheap, right almost always) ;
      # if that aggregate doesn't declare the command, search every
      # aggregate the domain actually has for one that does, and use THAT
      # instead of guessing wrong silently. Falls back to the original
      # guess (so the resulting UnknownVerb names the aggregate the author
      # meant) only if truly no aggregate declares it.
      # The real corpus writes VO-typed `expect` values BOTH ways: bare
      # (`expect sweeper_id: "fleet"`) and wrapped (`expect repo:
      # {value: "..."}`). A live record's field always comes back as a
      # Hecks::Runtime::Value ; unwrapping ONLY the actual side (the
      # obvious first fix) broke every wrapped-form expectation the other
      # way -- found live sweeping the real corpus (i745), not guessed.
      # Normalizing BOTH sides to the same bare-scalar-or-plain-hash shape
      # is the one comparison that accepts either spelling.
      def normalize(value)
        return Hecks::Runtime::Value.materialize_unwrapped(value) if value.is_a?(Hecks::Runtime::Value)
        return normalize(value[:value]) if value.is_a?(Hash) && value.keys == [:value]

        value
      end

      def pass_result(test, pending: false)
        { description: test.description, status: pending ? "pending" : "pass", message: nil }
      end

      def fail_result(test, message)
        { description: test.description, status: "fail", message: message }
      end

      def error_result(test, message)
        { description: test.description, status: "error", message: message }
      end
    end
  end
end
