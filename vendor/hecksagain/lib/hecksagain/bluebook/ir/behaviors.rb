module Hecksagain
  module Bluebook
    module IR
      # Vendored addition, not (yet) upstream hecksagain (i745 — build the
      # behaviors runtime BEFORE deleting the Rust parser, per Chris's
      # explicit order). Mirrors rust/src/behaviors_ir.rs's shape, ported
      # from the Rust reference (behaviors_parser.rs / behaviors_runner.rs)
      # rather than invented — the grammar is unchanged, only the reader is
      # new.
      #
      # One real simplification versus the Rust port: a `.behaviors` file is
      # RUBY, executed by `Kernel.load` exactly like every other bluebook
      # format here — `input agent_id: "x"` is a real keyword-arg call, not
      # text a parser has to re-typed. Rust needed a hand-rolled line parser
      # (behaviors_parser.rs) plus a string->Value coercer
      # (to_runtime_attrs) because Rust has no Ruby interpreter to lean on.
      # Neither exists here — the values already arrive typed.
      TestSetup = Struct.new(:command, :args, keyword_init: true)

      TestCase = Struct.new(:description, :tests_command, :on_aggregate, :kind,
                             :setups, :input, :expect, keyword_init: true) do
        # The FQN construction rust/src/behaviors_runner.rs:247-251 uses,
        # verbatim: an already-dotted tests_command is taken as a literal
        # FQN ; otherwise `on:` + the domain name (known only once a
        # runtime is booted, so it's a parameter here, not a field) compose
        # "Domain::Aggregate.Command".
        def fqn(domain_name)
          tests_command.to_s.include?(".") ? tests_command.to_s : "#{domain_name}::#{on_aggregate}.#{tests_command}"
        end

        def pending?    = kind == :pending
        def query?      = kind == :query
        def cross_cascade? = kind == :cross_cascade
      end

      BehaviorsSuite = Struct.new(:name, :vision, :tests, keyword_init: true)
    end
  end
end
