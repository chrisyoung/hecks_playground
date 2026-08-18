module Hecksagain
  module Bluebook
    module DSL
      class AdapterBuilder
        def initialize(name)
          @name    = name
          @fields  = []
          @secrets = []
        end

        def port(value) = @port = value.to_s

        def field(name) = @fields << name.to_sym

        def secret(name) = @secrets << name.to_sym

        # `handler "path/to/program"` — the out-of-process program
        # `bin/adapter-host` execs when this adapter's bound event fires
        # (Bluebook::Hexagon::Adapter#handler, documented there as "not yet
        # wired" on the consumer side — but the DSL word must still exist so
        # a `.adapter` file that declares it parses at all; the Stripe
        # example this gem ships is written exactly this way).
        def handler(value) = @handler = value.to_s

        def build
          Adapter.new(name: @name, port: @port, fields: @fields, secrets: @secrets, handler: @handler)
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
