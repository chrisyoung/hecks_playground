# HecksagainRuntime::BehaviorsRunner::DSL
#
# The `.behaviors` authoring grammar this repo's corpus is written in,
# plus the `Hecks.behaviors "Name" do ... end` seam that binds it.
#
# WHY THIS FILE EXISTS AT ALL. It used to live in the vendored hecks
# copy (vendor/hecks/lib/hecks/bluebook/behaviors.rb + dsl/
# behaviors_builder.rb) and stored its result in
# `Hecksagain.last_behaviors_suite`. That vendored copy is gone, and the
# published gem's replacement -- `Hecks::Behaviors` (hecks-1.3.0,
# lib/hecks/behaviors*), reachable via `require "hecks/behaviors"` and
# driven by `Hecks::Behaviors.run(path)` / `.run_all(dir)` -- is a
# DIFFERENT grammar, not a rename:
#
#   * it REQUIRES a `loads "..."` line naming the files to boot ; not one
#     of this repo's 102 `.behaviors` files has one (scope here is the
#     same-stem sibling `.bluebook`, resolved by the runner),
#   * `kind:` carries only `:query` ; this corpus also uses `:cascade`
#     (42), `:cross_cascade` (42), `:pending` (1) and `:driving_tick` (1),
#   * `expect ok:` accepts only boolean `true` ; this corpus writes the
#     string `"true"` 210 times,
#   * it boots ONE runtime per suite and resets state between tests ;
#     this runner boots a fresh runtime per test against a per-test
#     staged directory, which is what `:cross_cascade` scoping needs.
#
# So the gem cannot run this corpus as written, and adopting it would
# mean rewriting all 102 files rather than fixing a runner. The grammar
# below is therefore restored verbatim from the retired vendored copy and
# now OWNED here, beside the runner that reads it. Everything the runner
# actually executes against -- boot, dispatch, query, refusals, values --
# is the gem's real API (`Hecks.boot`, `Hecks::Runtime::*`) ; only the
# test-file grammar is local.
module HecksagainRuntime
  module BehaviorsRunner
    TestSetup = Struct.new(:command, :args, keyword_init: true)

    # `kind:` is the one field the gem's own TestCase has no equivalent
    # for beyond `:query` -- see this file's header. `fqn` composes
    # "Domain::Aggregate.Command" from `on:` plus the domain name, which
    # is known only once a runtime is booted, so it is a parameter here
    # rather than a field.
    TestCase = Struct.new(:description, :tests_command, :on_aggregate, :kind,
                          :setups, :input, :expect, keyword_init: true) do
      def fqn(domain_name)
        tests_command.to_s.include?(".") ? tests_command.to_s : "#{domain_name}::#{on_aggregate}.#{tests_command}"
      end

      def pending?       = kind == :pending
      def query?         = kind == :query
      def cross_cascade? = kind == :cross_cascade
      def cascade?       = kind == :cascade
      def driving_tick?  = kind == :driving_tick
    end

    BehaviorsSuite = Struct.new(:name, :vision, :tests, :path, keyword_init: true)

    # The authoring surface plus the two slots `BehaviorsRunner.parse`
    # binds around a `Kernel.load`. `loading_path` is set only for the
    # duration of one load (so a `.behaviors` file opened any other way
    # refuses rather than half-working) and `last_suite` is reset to nil
    # BEFORE every load -- the gem's own hard-won lesson, ported: a
    # runner that only compares `last_suite` against nil reports the
    # PREVIOUS file's suite for any later file that failed to build one.
    module DSL
      class << self
        attr_accessor :loading_path, :last_suite
      end

      class TestCaseBuilder
        def initialize(description)
          @description   = description
          @tests_command = nil
          @on_aggregate  = nil
          @kind          = nil
          @setups        = []
          @input         = {}
          @expect        = {}
        end

        def tests(command, on: nil, kind: nil)
          @tests_command = command
          @on_aggregate  = on
          @kind          = kind
        end

        def setup(command, **kwargs)
          @setups << TestSetup.new(command: command, args: kwargs)
        end

        def input(**kwargs)  = @input.merge!(kwargs)
        def expect(**kwargs) = @expect.merge!(kwargs)

        def build
          TestCase.new(description: @description, tests_command: @tests_command,
                       on_aggregate: @on_aggregate, kind: @kind,
                       setups: @setups, input: @input, expect: @expect)
        end
      end

      class BehaviorsBuilder
        def initialize(name, source_path:)
          @name        = name
          @source_path = source_path
          @vision      = nil
          @tests       = []
        end

        def vision(text) = @vision = text

        def test(description, &block)
          builder = TestCaseBuilder.new(description)
          builder.instance_eval(&block) if block
          @tests << builder.build
        end

        def build
          BehaviorsSuite.new(name: @name, vision: @vision, tests: @tests, path: @source_path)
        end

        def self.build(name, source_path:, &block)
          builder = new(name, source_path: source_path)
          builder.instance_eval(&block) if block
          builder.build
        end
      end
    end
  end
end

# THE SEAM. Every `.behaviors` file in this repo opens with
# `Hecks.behaviors "Name" do ... end`, so the name has to be bound on the
# gem's module. `require "hecks"` deliberately does NOT load
# lib/hecks/behaviors.rb (the gem says so in that file's own header), so
# nothing is being shadowed today -- but if that ever changes, the two
# incompatible grammars would swap silently, which is precisely the class
# of bug this whole fix is about. Hence the guard below, and the matching
# source_location check in `BehaviorsRunner.parse`.
if Hecks.respond_to?(:behaviors)
  raise "Hecks.behaviors is already defined by #{Hecks.method(:behaviors).source_location&.first.inspect} — " \
        "hecks's own behaviors DSL and this repo's are different grammars (see behaviors_runner/dsl.rb) " \
        "and must not be loaded into the same process"
end

module Hecks
  class << self
    # Deliberately NOT routed through `collect` (the way `bluebook`/
    # `hecksagon`/`world` are): a behaviors suite is a test artifact a
    # runner reads on demand, never something a live domain boot needs,
    # so it has no business landing in a Registry.
    def behaviors(name, &)
      dsl  = HecksagainRuntime::BehaviorsRunner::DSL
      path = dsl.loading_path or
        raise "Hecks.behaviors called outside BehaviorsRunner.parse — a .behaviors file " \
              "is only ever loaded by the behaviors runner"

      dsl.last_suite = dsl::BehaviorsBuilder.build(name, source_path: path, &)
    end
  end
end
