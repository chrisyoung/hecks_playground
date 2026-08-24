# Hecks::Bluebook::DSL::DrivingAdapterBuilder
#
# Builds the handlers declared inside `HecksagonBuilder#adapter "Name" do
# ... end` — a naming block a hecksagon uses to group handlers by which
# adapter they belong to, distinct from `Hecks.adapter "Name" do ... end`
# (AdapterBuilder, a different file entirely, which declares the adapter
# TYPE itself).
#
# THE DRIVEN SIDE (real, below): `driven on "Domain::Aggregate.Event" do
# dispatch "Domain::Aggregate.Command", with: { field: "{event_field}" } ;
# success "..." ; failure "..." end` — a direct in-process cross-context
# call, no port, no family, no adapter contract, captured onto Bluebook::
# DrivenHandler and read by Runtime::Dispatcher#record_driven_dispatches.
# `with:` mirrors a policy's own `trigger ... with: { ... }` — the same
# named-pairs shape for "extra literal payload attributes," not a
# bespoke bare-kwargs convention this file would be the only word to use.
# `for_each:` mirrors a policy's own `for_each` the same way — one
# dispatch per row a named query answers, instead of one per event.
#
# THE DRIVING SIDE — `driving on cron "*/10 * * * *" do |clock| dispatch
# "Domain::Aggregate.Command", field: "value" end` — an external clock
# reaching IN, the inverse of the driven side above. Grammar only: this
# builder captures `kind`/`arg`/`dispatch_command` onto Bluebook::
# DrivingHandler (already carried and merged correctly across a
# multi-file hecksagon per that struct's own comment in hexagon.rb, ahead
# of any DSL populating it), same shape `driven` already closes for the
# event-in direction. No runtime scheduler or `storehouse drive` CLI verb
# exists yet to actually FIRE these on their `kind`/`arg` schedule — that
# remains a separate, still-pending unit, same as before this closed the
# NoMethodError. `dispatch`'s kwargs (field interpolation) are accepted
# and discarded here, mirroring driven's own known, documented gap (the
# corpus's `driving on interval` blocks pass them but nothing downstream
# reads them yet — not attempted here, out of scope for closing the
# parse-time crash).
module Hecks
  module Bluebook
    module DSL
      class DrivingAdapterBuilder
        DRIVING_KINDS = %w[cron interval http_post file_watch].freeze

        # What `driven`'s own block accumulates — `dispatch`'s command +
        # its `{field}` interpolation map, and the optional `success`/
        # `failure` verdict commands. Not a `Bind`: a driven handler names
        # no port and no adapter contract, only the adapter block's own
        # label (threaded in by `driven`, below).
        class DrivenCapture
          attr_reader :dispatch_command, :dispatch_args, :for_each, :success_command, :failure_command

          # THE FAN-OUT, on the DRIVEN side — `for_each: { from:
          # "Domain::Aggregate.query", where: { field: "{event_field}" } }`
          # turns this one dispatch into one dispatch PER ROW the named
          # query answers, exactly as `Policy#for_each` (PolicyInterpreter
          # #deliver_for_each) and a process manager's own `dispatch ...,
          # for_each:` (SagaInterpreter#deliver_saga_dispatch) already do
          # one layer over. Captured verbatim here and resolved at
          # delivery time by Runtime::Dispatcher#fan_out_driven — the
          # builder knows no registry, so it cannot resolve a query.
          #
          # `where:` is the QUERY'S OWN arguments, `{field}`-interpolated
          # against the triggering event (never against a row — the query
          # is what DECIDES the rows). Declaring it explicitly is the one
          # deliberate difference from a Policy's `for_each`, which has no
          # slot to say it in and so forwards the whole event payload: a
          # driven fan-out routinely names a no-argument query (`where:
          # {}`), and forwarding a payload it never declared would refuse
          # every sweep with UnknownArgument.
          def dispatch(command, with: {}, for_each: nil)
            @dispatch_command = command.to_s
            @dispatch_args    = with
            @for_each         = for_each
          end

          def success(command) = @success_command = command.to_s
          def failure(command) = @failure_command = command.to_s
        end

        # The driving side's own capture — just the dispatch command FQN.
        # `**` swallows any field-mapping kwargs without reading them (see
        # header comment: a documented, unfixed gap shared with driven).
        class DrivingDispatchCapture
          attr_reader :dispatch_command

          def dispatch(command, **) = @dispatch_command = command.to_s
        end

        attr_reader :driven_handlers, :driving_handlers

        def initialize(name)
          @name              = name.to_s
          @driven_handlers   = []
          @driving_handlers  = []
        end

        # Pure readability sugar — `driven on "..."`/`driving on cron
        # "..."` both read as a sentence; `on` itself does nothing but
        # hand back what it was given (a bare event string for `driven`,
        # a [kind, arg] pair for `driving`, from the DRIVING_KINDS helpers
        # below).
        def on(value) = value

        # `cron("*/10 * * * *")` / `interval("2s")` / etc. — Ruby parses
        # `driving on cron "x" do ... end` as `driving(on(cron("x"))) {
        # ... }`, so these tiny helpers just hand `on` a [kind, arg] pair
        # to pass straight through to `driving`.
        DRIVING_KINDS.each do |kind|
          define_method(kind) { |arg| [kind, arg] }
        end

        def driving((kind, arg), &block)
          capture = DrivingDispatchCapture.new
          capture.instance_eval(&block) if block

          @driving_handlers << DrivingHandler.new(
            adapter_name:     @name,
            kind:             kind,
            arg:              arg,
            dispatch_command: capture.dispatch_command
          )
        end

        def driven(event, &block)
          capture = DrivenCapture.new
          capture.instance_eval(&block) if block

          @driven_handlers << DrivenHandler.new(
            adapter_name:     @name,
            event:            event.to_s,
            dispatch_command: capture.dispatch_command,
            dispatch_args:    capture.dispatch_args || {},
            for_each:         capture.for_each,
            success:          capture.success_command,
            failure:          capture.failure_command
          )
        end

        def self.build(name, &block)
          builder = new(name)
          builder.instance_eval(&block) if block
          builder
        end
      end
    end
  end
end
