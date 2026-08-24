module Hecks
  module Bluebook
    module DSL
      class PortBuilder
        def initialize(name)
          @name   = name
          @signal = :reply
        end

        def verb(value)   = @verb = value.to_s
        def signal(value) = @signal = value.to_sym

        # Vendored addition, not (yet) upstream hecks (parser-removal
        # plan, Phase 1a). See Port's own comment for why this exists.
        def produces(name) = @produces = name.to_sym

        def build
          Port.new(name: @name, verb: @verb, signal: @signal, produces: @produces)
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
