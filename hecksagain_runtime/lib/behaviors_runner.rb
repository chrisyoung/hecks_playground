# HecksagainRuntime::BehaviorsRunner
#
# The impure edge that actually EXECUTES a `.behaviors` suite: finds the
# suite's source `.bluebook`, boots a fresh in-memory runtime scoped to it,
# dispatches each test's setups + command, checks the expectations, tears
# the runtime down. Ported from rust/src/behaviors_runner.rs (i745 -- build
# this BEFORE the Rust parser is deleted, per Chris's explicit order), NOT
# invented: same grammar, same per-test isolation model ("each test gets a
# fresh runtime so state doesn't leak across tests"), same domain-scope
# rule (a `:cross_cascade` test sees the full corpus ; every other kind
# sees ONLY its own source bluebook, so isolated tests can't see
# sibling-file cascades).
#
# The `.behaviors` GRAMMAR itself lives in behaviors_runner/dsl.rb -- read
# that file's header for why it is owned here rather than taken from the
# published gem's own `Hecks::Behaviors`.
#
# KNOWN DEVIATION from the Rust reference, disclosed rather than papered
# over: hecks's Dispatcher#dispatch has no cascade-off mode (no
# `dispatch_isolated` equivalent), so every dispatch here -- setups
# included -- runs with cascades ON. For an isolated test (source bluebook
# alone, no siblings) this usually changes nothing, since a policy needs
# another aggregate to react against ; it becomes a real risk only for a
# bluebook whose own aggregates chain via policies internally.
#
# PARSE ERROR vs TEST ERROR. These are two different failures and this
# file keeps them apart. A `parse_error` means the FILE never produced a
# suite: it raised while being `Kernel.load`ed, or it loaded without ever
# calling `Hecks.behaviors`. Anything that goes wrong once real tests are
# running -- a boot, a dispatch, a refusal -- is that TEST's `error`
# status, never the file's. The old `rescue StandardError` wrapped both
# halves, so one bad dispatch could bury an entire suite's worth of
# results under a single "parse error" line.
#
# Usage:
#   HecksagainRuntime::BehaviorsRunner.run_corpus("/path/to/aggregates")
#   HecksagainRuntime::BehaviorsRunner.run_file("/path/to/x.behaviors")

require "tmpdir"
require "fileutils"
require_relative "behaviors_runner/dsl"
require_relative "behaviors_runner/expectations"

module HecksagainRuntime
  module BehaviorsRunner
    module_function

    # `Kernel.load`s one `.behaviors` file and returns its suite, with NO
    # test executed yet. `DSL.loading_path` is bound only for the duration
    # of the load, and `last_suite` is reset to nil BEFORE it, so a file
    # that loads without calling `Hecks.behaviors` is unambiguously a
    # parse error and never the previous file's suite.
    #
    # `ScriptError` is rescued alongside `StandardError` on purpose: a
    # `.behaviors` file is Ruby, so a syntax error in one is a SyntaxError
    # -- not a StandardError -- and would otherwise abort the whole sweep
    # instead of failing the one file that is broken.
    def parse(path)
      guard_seam!
      previous = DSL.loading_path
      DSL.loading_path = File.expand_path(path)
      DSL.last_suite   = nil

      begin
        Kernel.load(path)
      rescue StandardError, ScriptError => e
        return [nil, "#{e.class}: #{e.message}"]
      ensure
        DSL.loading_path = previous
      end

      suite = DSL.last_suite
      return [nil, "file loaded but called no Hecks.behaviors"] unless suite

      [suite, nil]
    end

    def run_file(path, corpus_root: nil, staged_root: nil)
      source = source_bluebook_for(path)
      return file_result(path, nil, "no matching .bluebook for #{path}") unless source

      suite, parse_error = parse(path)
      return file_result(path, source, parse_error) if parse_error

      runs = suite.tests.map do |test|
        run_test(test, source: source, corpus_root: corpus_root, staged_root: staged_root)
      end
      { suite: path, source: source, parse_error: nil, runs: runs }
    end

    # The suite parsed, so whatever happens from here belongs to ONE test,
    # not to the file. `Expectations.run_one` already answers an `error`
    # result for anything it catches ; this is the backstop for anything
    # that escapes it (a ScriptError from a lazily-required adapter, say),
    # so a single unlucky test can never be re-reported as a parse error.
    def run_test(test, source:, corpus_root:, staged_root:)
      Expectations.run_one(test, source: source, corpus_root: corpus_root, staged_root: staged_root)
    rescue StandardError, ScriptError => e
      { description: test.description, status: "error", message: "#{e.class}: #{e.message}" }
    end

    # Sweeps every `.behaviors` file under root. Prints the file count it
    # actually found -- i745's own lesson: a gate that reports green
    # without saying how much it looked at is the failure mode this whole
    # arc exists to end. The staged flat-corpus dir (for any :cross_cascade
    # tests) is built ONCE and shared across the sweep -- same files, only
    # the runtime state resets per test.
    def run_corpus(root)
      files = Dir.glob(File.join(root, "**", "*.behaviors")).sort
      staged = HecksagainRuntime.stage_flat_corpus(root)
      results = files.map { |f| run_file(f, corpus_root: root, staged_root: staged) }
      { root: root, files_swept: files.size, files: results, summary: summarize(results) }
    ensure
      FileUtils.remove_entry(staged) if staged && staged != root && File.exist?(staged)
    end

    def summarize(results)
      runs = results.flat_map { |r| r[:runs] || [] }
      {
        files: results.size,
        parse_errors: results.count { |r| r[:parse_error] },
        total: runs.size,
        passed: runs.count { |r| r[:status] == "pass" },
        failed: runs.count { |r| r[:status] == "fail" },
        errored: runs.count { |r| r[:status] == "error" },
        pending: runs.count { |r| r[:status] == "pending" }
      }
    end

    def source_bluebook_for(behaviors_path)
      dir = File.dirname(behaviors_path)
      stem = File.basename(behaviors_path, ".behaviors")
      candidate = File.join(dir, "#{stem}.bluebook")
      File.file?(candidate) ? candidate : nil
    end

    def file_result(path, source, parse_error)
      { suite: path, source: source, parse_error: parse_error, runs: [] }
    end

    # See dsl.rb's own seam comment: if anything ever loads the gem's
    # `hecks/behaviors` after this file, `Hecks.behaviors` would silently
    # become a different, incompatible grammar and every file in the sweep
    # would report "no `loads`". Cheaper to check once per parse than to
    # debug 102 identical parse errors again.
    def guard_seam!
      owner = Hecks.method(:behaviors).source_location&.first
      return if owner == File.expand_path("behaviors_runner/dsl.rb", __dir__)

      raise "Hecks.behaviors is bound to #{owner.inspect}, not this repo's .behaviors grammar " \
            "(hecksagain_runtime/lib/behaviors_runner/dsl.rb)"
    end
  end
end
