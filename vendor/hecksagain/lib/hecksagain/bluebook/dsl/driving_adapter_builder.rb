module Hecksagain
  module Bluebook
    module DSL
      # Vendored addition, not (yet) upstream hecksagain (migration plan
      # task 7). Builds the body of `adapter "Name" do driving on <kind>
      # "<arg>" do |signal| dispatch "Domain::Aggregate.Command" end end`
      # -- ported from rust/src/hecksagon_parser.rs::parse_driving_handler
      # and ruby/hecksagon/dsl/driven_adapter_builder.rb (two other Hecks
      # codebases already proving this exact grammar), not invented fresh.
      #
      # `on` is a bare method taking the schedule/kind keyword
      # (cron/interval/http_post/file_watch) as its NAME and the schedule
      # arg as its sole positional -- Ruby parses `on cron "*/10 * * * *"`
      # as `on(cron("*/10 * * * *"))`, so `cron`/`interval`/`http_post`/
      # `file_watch` are themselves tiny helper methods returning a
      # [kind, arg] pair for `on` to unpack, mirroring how the DSL text
      # reads (`driving on cron "..."`, not `driving(on: "cron", ...)`).
      class DrivingAdapterBuilder
        DRIVING_KINDS = %w[cron interval http_post file_watch].freeze

        def initialize(adapter_name)
          @adapter_name = adapter_name
          @handlers     = []
        end

        # Ruby's do/end binds to the OUTERMOST call in an unparenthesized
        # chain: `driving on cron "x" do |clock| ... end` is
        # `driving( on(cron("x")) ) { |clock| ... }` -- the block goes to
        # `driving`, not `on`. `on` just returns the [kind, arg] pair `on`
        # was handed by `cron(...)`/`interval(...)`/etc.
        DRIVING_KINDS.each do |kind|
          define_method(kind) { |arg| [kind, arg] }
        end

        def on(kind_arg_pair) = kind_arg_pair

        def driving((kind, arg), &block)
          dispatcher = DispatchCapture.new
          dispatcher.instance_eval(&block) if block
          @handlers << IR::DrivingHandler.new(
            adapter_name: @adapter_name, kind: kind, arg: arg,
            dispatch_command: dispatcher.command
          )
        end

        # Vendored catch-all no-op, not (yet) upstream hecksagain
        # (migration plan task 4): `adapter :compute, name: :x do
        # function "..." trigger_on "Event" response_into "Cmd", attr:
        # :field end` / `adapter :llm do prompt_template "..." ... end` —
        # genuinely DIFFERENT shapes from `driving on cron/interval` (a
        # custom adapter's own config keywords, not a clock tick), found
        # live in miette's dream/lucid-dream/voice hecksagons. Real
        # methods (`driving`/`on`/the DRIVING_KINDS helpers above) still
        # win over method_missing, so this only absorbs the keywords THIS
        # vendored port doesn't know — every custom-adapter-kind
        # configuration verb, not one at a time, since the actual set is
        # open-ended (Part 4's "5 custom-adapter-kind files" bucket:
        # :compute/:dream_image/:dream_translate/:exec/:lucid_steer/:llm,
        # each with its own config vocabulary). Structurally captured, not
        # wired — a real fix means designing and implementing each custom
        # adapter kind's actual runtime behavior, not attempted here.
        # TODO upstream via bin/evolve (migration plan task 7).
        def method_missing(*) = nil
        def respond_to_missing?(*) = true

        def build = @handlers

        def self.build(adapter_name, &block)
          builder = new(adapter_name)
          builder.instance_eval(&block) if block
          builder.build
        end

        # Captures the single `dispatch "Domain::Aggregate.Command"` call
        # inside a driving handler's block -- the block's own `|signal|`
        # param carries no data this vendored version reads (a real,
        # documented gap: the old Rust runtime passed clock-tick metadata
        # into `signal`; this structural port doesn't thread it through
        # yet). TODO upstream via bin/evolve (migration plan task 7).
        class DispatchCapture
          attr_reader :command

          def dispatch(command_fqn, **) = @command = command_fqn
        end
      end
    end
  end
end
