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

        # Vendored no-op stub, not (yet) upstream hecksagain (parser-removal
        # plan, Phase 1a): `handler "path/to/program"` names the
        # out-of-process program the adapter-host execs when the bound
        # event fires (examples/pizzas/bluebook/stripe.adapter, several
        # adapters/*/*.adapter files). Accepted so a file that documents its
        # own handler path boots, matching the codebase's own established
        # convention (HecksagonBuilder#success/#failure before this pass) --
        # not wired to anything real yet. Real handler resolution (a
        # dump-hecksagon-equivalent CLI facility) is i746's job, step 7.
        def handler(*) = nil

        def build
          IR::Adapter.new(name: @name, port: @port, fields: @fields, secrets: @secrets)
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
