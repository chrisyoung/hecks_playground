# The authoring surface for `Hecks.behaviors "Name" do ... end` — one
# `test "description" do ... end` block per case, `tests`/`setup`/
# `input`/`expect` inside. Mirrors WorldBuilder's own
# `self.build(name, &block)` / `instance_eval` shape exactly.
#
# NOT collected into the domain Registry the way `bluebook`/`hecksagon`/
# `world` are (`Hecks.behaviors`, lib/hecksagain.rb, skips `collect`) — a
# behaviors suite is a test artifact the RUNNER reads, never a thing a
# live domain boot needs. See `Bluebook::BehaviorsSuite`'s own header for
# the runner boundary this stays on the authoring side of.
module Hecksagain
  module Bluebook
    module DSL
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
        def initialize(name)
          @name   = name
          @vision = nil
          @tests  = []
        end

        def vision(text) = @vision = text

        def test(description, &block)
          builder = TestCaseBuilder.new(description)
          builder.instance_eval(&block) if block
          @tests << builder.build
        end

        def build
          BehaviorsSuite.new(name: @name, vision: @vision, tests: @tests)
        end

        def self.build(name, &block)
          builder = new(name)
          builder.instance_eval(&block) if block
          builder.build
        end
      end
    end
  end
end
