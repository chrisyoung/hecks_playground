# HecksPlayground::ArchitectureTour::Steps
#
# Step definitions for the architecture tour. Each method returns a
# Step struct with title, explanation, and relevant file paths.
#
#   include Steps
#   monorepo_layout_step  # => ArchitectureTour::Step
#
module HecksPlayground
  class ArchitectureTour
    # HecksPlayground::ArchitectureTour::Steps
    #
    # Step definitions for the architecture tour: each method returns a Step with title, explanation, and paths.
    #
    module Steps
      def monorepo_layout_step
        Step.new(
          title: "Monorepo layout",
          explanation: "HecksPlayground is a monorepo with six components, each its own gem.\n" \
                       "hecksagon is the runtime, hecks_playground_workshop is the REPL,\n" \
                       "hecksties is the CLI glue, hecks_playground_targets has code generators,\n" \
                       "hecks_playground_ai provides MCP/AI tooling, and hecks_playground_on_rails is the Rails adapter.",
          paths: %w[
            hecksagon/
            hecks_playground_workshop/
            hecksties/
            hecks_playground_targets/
            hecks_playground_ai/
            hecks_playground_on_rails/
          ]
        )
      end

      def bluebook_dsl_step
        Step.new(
          title: "Bluebook DSL",
          explanation: "The Bluebook is the DSL source of truth. Domain definitions are\n" \
                       "written in Ruby blocks and parsed by builders into an IR.\n" \
                       "Validators enforce DDD rules before anything is generated.",
          paths: %w[
            hecksagon/lib/hecksagon/bluebook/
            hecksagon/lib/hecksagon/bluebook/builders/
            hecksagon/lib/hecksagon/bluebook/validators/
          ]
        )
      end

      def hecksagon_ir_step
        Step.new(
          title: "Hecksagon IR (intermediate representation)",
          explanation: "The Bluebook DSL compiles into an IR of plain Ruby structs:\n" \
                       "Domain, Aggregate, Attribute, Command, Event, Policy, etc.\n" \
                       "Every downstream tool (generators, MCP, CLI) reads this IR.",
          paths: %w[
            hecksagon/lib/hecksagon/domain.rb
            hecksagon/lib/hecksagon/aggregate.rb
            hecksagon/lib/hecksagon/attribute.rb
            hecksagon/lib/hecksagon/command.rb
          ]
        )
      end

      def compiler_pipeline_step
        Step.new(
          title: "Compiler pipeline",
          explanation: "The compiler reads a domain.rb file, builds the IR via Bluebook,\n" \
                       "validates it, then hands it to a target (Ruby, Go, Node) for\n" \
                       "code generation. Each target uses contracts to guarantee parity.",
          paths: %w[
            hecksagon/lib/hecksagon/compiler.rb
            hecks_playground_targets/lib/hecks_playground_targets/
          ]
        )
      end

      def hecksties_glue_step
        Step.new(
          title: "Hecksties glue layer",
          explanation: "Hecksties wires everything together: the Thor-based CLI,\n" \
                       "command registration, domain helpers, and the import system\n" \
                       "that reverse-engineers Rails apps into HecksPlayground domains.",
          paths: %w[
            hecksties/lib/hecks_playground_cli/cli.rb
            hecksties/lib/hecks_playground_cli/commands/
            hecksties/lib/hecks_playground_cli/import.rb
          ]
        )
      end

      def code_generators_step
        Step.new(
          title: "Code generators and targets",
          explanation: "Each target generates a full application from the IR.\n" \
                       "The Ruby target produces a gem with aggregates, commands,\n" \
                       "repositories, and a Sinatra server. Go and Node targets\n" \
                       "follow the same contracts for cross-target parity.",
          paths: %w[
            hecks_playground_targets/lib/hecks_playground_targets/ruby_target/
            hecks_playground_targets/lib/hecks_playground_targets/go_target/
            hecks_playground_targets/lib/hecks_playground_targets/node_target/
          ]
        )
      end

      def workshop_step
        Step.new(
          title: "Workshop (interactive REPL)",
          explanation: "The workshop provides sketch mode (design aggregates, commands,\n" \
                       "lifecycles) and play mode (execute commands, query data, inspect\n" \
                       "events). `hecks_playground console` launches it; `hecks_playground tour` demos it.",
          paths: %w[
            hecks_playground_workshop/lib/hecks_playground/workshop.rb
            hecks_playground_workshop/lib/hecks_playground/workshop/sketch_mode.rb
            hecks_playground_workshop/lib/hecks_playground/workshop/play_mode.rb
            hecks_playground_workshop/lib/hecks_playground/workshop/tour.rb
          ]
        )
      end

      def ai_tools_step
        Step.new(
          title: "AI tools (MCP server)",
          explanation: "hecks_playground_ai exposes the domain compiler as an MCP server so AI\n" \
                       "agents can create aggregates, add commands, validate, and build\n" \
                       "domains through tool calls. `hecks_playground mcp` starts the server.",
          paths: %w[
            hecks_playground_ai/lib/hecks_playground_ai/
            hecks_playground_ai/lib/hecks_playground_ai/mcp_server.rb
          ]
        )
      end

      def cli_registration_step
        Step.new(
          title: "CLI command registration",
          explanation: "Each component registers commands via HecksPlayground::CLI.register_command.\n" \
                       "The CLI auto-discovers commands from hecks_playground*/lib/**/commands/*.rb,\n" \
                       "groups them by component, and installs them as Thor methods.",
          paths: %w[
            hecksties/lib/hecks_playground_cli/cli.rb
            hecks_playground_workshop/lib/hecks_playground/workshop/commands/tour.rb
            hecksties/lib/hecks_playground_cli/commands/
          ]
        )
      end

      def spec_conventions_step
        Step.new(
          title: "Spec conventions",
          explanation: "Tests use memory adapters for speed (suite must run under 1 second).\n" \
                       "Each component has its own spec/ directory. Integration specs live\n" \
                       "in hecksties/spec/. Stub $stdin.tty? for interactive features.",
          paths: %w[
            hecksagon/spec/
            hecks_playground_workshop/spec/
            hecksties/spec/
            hecks_playground_targets/spec/
          ]
        )
      end

      private

      def build_steps
        [
          monorepo_layout_step,
          bluebook_dsl_step,
          hecksagon_ir_step,
          compiler_pipeline_step,
          hecksties_glue_step,
          code_generators_step,
          workshop_step,
          ai_tools_step,
          cli_registration_step,
          spec_conventions_step
        ]
      end
    end
  end
end
