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
# KNOWN DEVIATION from the Rust reference, disclosed rather than papered
# over: hecksagain's Dispatcher#dispatch has no cascade-off mode (no
# `dispatch_isolated` equivalent -- checked directly, runtime/dispatcher.rb
# always pumps @policies.react + @sagas.advance). Every dispatch here --
# setups included -- runs with cascades ON. For an isolated test (source
# bluebook alone, no siblings) this usually changes nothing, since a policy
# needs another aggregate to react against ; it becomes a real risk only for
# a bluebook whose own aggregates chain via policies internally. The sweep
# results report which tests this could plausibly affect.
#
# Usage:
#   HecksagainRuntime::BehaviorsRunner.run_corpus("/path/to/aggregates")
#   HecksagainRuntime::BehaviorsRunner.run_file("/path/to/x.behaviors")

require "tmpdir"
require "fileutils"
require_relative "behaviors_runner/expectations"

module HecksagainRuntime
  module BehaviorsRunner
    module_function

    def run_file(path, corpus_root: nil, staged_root: nil)
      source = source_bluebook_for(path)
      return { suite: path, source: nil, parse_error: "no matching .bluebook for #{path}", runs: [] } unless source

      Kernel.load(path)
      suite = Hecksagain.last_behaviors_suite
      unless suite
        return { suite: path, source: source, parse_error: "file loaded but called no Hecks.behaviors", runs: [] }
      end

      runs = suite.tests.map do |test|
        Expectations.run_one(test, source: source, corpus_root: corpus_root, staged_root: staged_root)
      end
      { suite: path, source: source, parse_error: nil, runs: runs }
    rescue StandardError => e
      { suite: path, source: source, parse_error: "#{e.class}: #{e.message}", runs: [] }
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
      FileUtils.remove_entry(staged) if staged && File.exist?(staged)
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
  end
end
