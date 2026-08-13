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
          @adapter_name     = adapter_name
          @driving_handlers = []
          @driven_handlers  = []
        end

        attr_reader :driving_handlers, :driven_handlers

        # Ruby's do/end binds to the OUTERMOST call in an unparenthesized
        # chain: `driving on cron "x" do |clock| ... end` is
        # `driving( on(cron("x")) ) { |clock| ... }` -- the block goes to
        # `driving`, not `on`. `on` just returns the [kind, arg] pair `on`
        # was handed by `cron(...)`/`interval(...)`/etc.
        DRIVING_KINDS.each do |kind|
          define_method(kind) { |arg| [kind, arg] }
        end

        # `on` is overloaded structurally between driving (a [kind, arg]
        # pair, from cron/interval/http_post/file_watch above) and driven
        # (a bare event-name string, `driven on "Domain::Aggregate.Event"`)
        # -- both just pass their argument straight through, so ONE method
        # serves both call shapes without needing to distinguish them.
        def on(kind_arg_pair_or_event) = kind_arg_pair_or_event

        def driving((kind, arg), &block)
          dispatcher = DispatchCapture.new
          dispatcher.instance_eval(&block) if block
          @driving_handlers << IR::DrivingHandler.new(
            adapter_name: @adapter_name, kind: kind, arg: arg,
            dispatch_command: dispatcher.command
          )
        end

        # i747 — `driven on "Domain::Aggregate.Event" do dispatch
        # "Domain::Aggregate.Command", field: "{event_field}" ; success
        # "..." ; failure "..." end`. Was silently swallowed whole by
        # method_missing before this (not even `dispatch` ran) — real now.
        def driven(event, &block)
          dispatcher = DrivenCapture.new
          dispatcher.instance_eval(&block) if block
          @driven_handlers << IR::DrivenHandler.new(
            adapter_name: @adapter_name, event: event.to_s,
            dispatch_command: dispatcher.command, dispatch_args: dispatcher.args,
            success: dispatcher.success_command, failure: dispatcher.failure_command
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

        # i747 -- returns the BUILDER, not a bare array : HecksagonBuilder
        # #adapter (the one caller) needs BOTH `driving_handlers` and
        # `driven_handlers` off one evaluation of `block`, and evaluating
        # the block twice (once per accessor) would double-run any driven/
        # driving declaration inside it. One instance_eval, two readers.
        def self.build(adapter_name, &block)
          builder = new(adapter_name)
          builder.instance_eval(&block) if block
          builder
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

        # i747 -- the DRIVEN-side capture, sibling to DispatchCapture above.
        # Unlike DispatchCapture, this KEEPS the kwargs `dispatch` receives
        # (`args`) -- i746/i747's own field mapping contract
        # (`dispatch "Banking::Account.Deposit", amount: "{amount}", ...`)
        # lives entirely in those kwargs, and DispatchCapture silently
        # dropping them was itself a live bug in the driving-side path
        # (unfixed here -- out of scope, driving's own dispatch_command
        # never carried field mappings in the corpus, so nothing observably
        # broke ; noted, not touched, to keep this change scoped to the
        # driven side i747 actually asked for).
        class DrivenCapture
          # success_command/failure_command, not success/failure -- those
          # names are the DSL VERBS themselves (below), called with one
          # arg to SET the value ; a same-named zero-arg reader would
          # collide with that arity the moment #driven reads it back.
          attr_reader :command, :args, :success_command, :failure_command

          def dispatch(command_fqn, **kwargs)
            @command = command_fqn
            @args    = kwargs
          end

          def success(cmd) = @success_command = cmd.to_s
          def failure(cmd) = @failure_command = cmd.to_s
        end
      end
    end
  end
end
