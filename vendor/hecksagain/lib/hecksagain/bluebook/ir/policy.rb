module Hecksagain
  module Bluebook
    module IR
      class Policy

        # The BLUEBOOK's name for this construct, asked the same way of a class
        # that has crossed over and of an IR object that has not. Collapses into
        # Construct when this one crosses.
        def hecks_name = @name
        attr_reader :name, :on_event, :trigger_command, :target_domain, :with_literals, :wheres, :for_each

        # WHICH HEAD DECLARED IT, or nil for one declared on the chapter.
        #
        # A policy written inside an aggregate is HOISTED onto the chapter by the
        # builder, and that is where the runtime reads it — so nothing was ever lost
        # at runtime. What was lost is the record of where it was written, which is
        # a fact about the source, and the language holds facts about the source.
        # Deliberately absent from `to_h`: the wire format is a pinned
        # contract, and it does not carry this.
        attr_accessor :aggregate

        # with_literals -- vendored addition, not (yet) upstream
        # hecksagain. hecks_conception's PolicyBuilder supports repeated
        # `with "key", "value"` calls supplying extra literal payload
        # attributes for the triggered command (5 files, e.g.
        # agent_instrumentation.bluebook's EstablishSidequestAgent).
        # TODO upstream via bin/evolve (migration plan task 7).
        def initialize(name:, on_event: nil, trigger_command: nil, target_domain: nil, aggregate: nil,
                       with_literals: {}, wheres: {}, for_each: nil)
          @name            = name.to_s
          @on_event        = on_event
          @trigger_command = trigger_command
          @target_domain   = target_domain
          @aggregate       = aggregate&.to_s
          @with_literals   = with_literals
          @wheres          = wheres
          @for_each        = for_each
        end

        def event_qualifier = Naming.qualifier(@on_event)

        def event_name = Naming.unqualified(@on_event)

        def to_h
          {
            name:            @name,
            on_event:        @on_event,
            trigger_command: @trigger_command,
            target_domain:   @target_domain
          }
        end
      end
    end
  end
end
