# Hecksagain::Bluebook::DSL::DrivingAdapterBuilder
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
# THE DRIVING SIDE (not yet built here): `driving on cron "*/10 * * * *"
# do |clock| dispatch "..." end` — an external clock reaching IN. Its own
# grammar and runtime scheduler are a separate, still-pending unit; no
# `driving` method is defined on this builder, so writing one today
# raises NoMethodError rather than silently doing nothing — the same
# failure mode this whole file exists to close for `driven`.
module Hecksagain
  module Bluebook
    module DSL
      class DrivingAdapterBuilder
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

        attr_reader :driven_handlers, :driving_handlers

        def initialize(name)
          @name              = name.to_s
          @driven_handlers   = []
          @driving_handlers  = []
        end

        # Pure readability sugar — `driven on "..."` reads as a sentence;
        # `on` itself does nothing but hand back what it was given.
        def on(value) = value

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
