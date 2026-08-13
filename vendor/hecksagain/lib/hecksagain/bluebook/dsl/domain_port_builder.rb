module Hecksagain
  module Bluebook
    module DSL
      class DomainPortBuilder
        def initialize(name, owner: nil)
          @name       = name
          @owner      = owner
          @operations = []
        end

        def operation(name, &block)
          @operations << PortOperationBuilder.build(name, owner: @owner, &block)
        end

        # THE DRIVEN HALF OF THE SAME WORD. `operation`/`emits` translates an
        # inbound fact into this domain's own event vocabulary — there is no
        # channel back to a caller beyond the events it emits. `verb` is the
        # opposite direction: the domain calling OUT to a swappable adapter
        # and getting a real value back (a checkout URL, a fetched document),
        # exactly what `Hecks.port "name" do verb "x" end` already builds —
        # this is that same `IR::Port`, reached from the same `port` call
        # `operation` already lives under, so a project's own resource ports
        # read next to their binding instead of in a separate file. One port,
        # one shape or the other — never both.
        def verb(value) = @verb = value.to_s

        # Vendored addition, not (yet) upstream hecksagain (parser-removal
        # plan, Phase 1a): the verb-shaped inline form (`port "X" do verb
        # "..." end`, inside a .hecksagon) previously hardcoded `signal:
        # :reply` unconditionally -- correct for a genuine reply port, but a
        # SILENT MIS-RECORD for an effect-family one declared this way (the
        # real corpus has 6 of 9 families needing :effect). Defaults to
        # :reply when unspecified, so every existing bare `verb "x"` caller
        # keeps its current behavior unchanged. `produces` mirrors the
        # top-level `PortBuilder`'s own addition -- see IR::Port's comment.
        def signal(value)   = @signal = value.to_sym
        def produces(value) = @produces = value.to_sym

        def build
          if @verb && !@operations.empty?
            raise Malformed, "#{@name} declares both a verb and operations — a port is one or the other, not both"
          end

          return IR::Port.new(name: @name, verb: @verb, signal: @signal || :reply, produces: @produces) if @verb

          raise Malformed, "#{@name} declares no verb and no operations" if @operations.empty?

          IR::DomainPort.new(name: @name, operations: @operations)
        end

        def self.build(name, owner: nil, &block)
          builder = new(name, owner: owner)
          builder.instance_eval(&block) if block
          builder.build
        end
      end
    end
  end
end
