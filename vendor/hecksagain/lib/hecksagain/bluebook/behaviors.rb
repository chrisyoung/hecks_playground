# Hecksagain::Bluebook::{TestSetup,TestCase,BehaviorsSuite}
#
# The `.behaviors` authoring surface's own IR — `Hecks.behaviors "Name" do
# ... end`. Mirrors storehouse's own Rust behaviors_ir.rs shape, ported
# from that reference rather than invented — the grammar is unchanged,
# only this reader is new to the Ruby gem.
#
# One real simplification versus the Rust port: a `.behaviors` file is
# RUBY, executed by `Kernel.load` exactly like every other bluebook format
# here — `input agent_id: "x"` is a real keyword-arg call, not text a
# parser has to re-type. Rust needed a hand-rolled line parser
# (behaviors_parser.rs) plus a string->Value coercer because it has no
# Ruby interpreter to lean on; neither exists here — the values already
# arrive typed.
#
# Deliberately NOT part of `include Hecksagain::IR`/`emits_ir` — a
# behaviors suite is never collected into the domain Registry (see
# `Hecks.behaviors`, lib/hecksagain.rb) and never dispatched through
# MetaValidator, so it carries none of the self-hosted round-trip
# machinery every real bluebook construct does. It is a test artifact a
# RUNNER reads on demand, not a domain a boot needs.
module Hecksagain
  module Bluebook
    TestSetup = Struct.new(:command, :args, keyword_init: true)

    TestCase = Struct.new(:description, :tests_command, :on_aggregate, :kind,
                           :setups, :input, :expect, keyword_init: true) do
      # An already-dotted tests_command is taken as a literal FQN;
      # otherwise `on:` + the domain name (known only once a runtime is
      # booted, so it's a parameter here, not a field) compose
      # "Domain::Aggregate.Command".
      def fqn(domain_name)
        tests_command.to_s.include?(".") ? tests_command.to_s : "#{domain_name}::#{on_aggregate}.#{tests_command}"
      end

      def pending?        = kind == :pending
      def query?          = kind == :query
      def cross_cascade?  = kind == :cross_cascade
    end

    BehaviorsSuite = Struct.new(:name, :vision, :tests, keyword_init: true)
  end
end
