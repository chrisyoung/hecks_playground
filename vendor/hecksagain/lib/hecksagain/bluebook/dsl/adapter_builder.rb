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

        # i746 step 7 — REAL now, not a stub. `handler "path/to/program"`
        # names the out-of-process program bin/adapter-host execs when the
        # bound event fires. Read by HecksagainRuntime.adapter_handler /
        # the `hecksagain-cli adapter-handler` subcommand.
        def handler(path) = @handler = path.to_s

        def build
          IR::Adapter.new(name: @name, port: @port, fields: @fields, secrets: @secrets, handler: @handler)
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
