# Hecks::Ports::Execution
#
# The EXECUTION port — the boundary a tool-invocation aggregate crosses to
# actually do the impure thing it names. `Tools::ShellTool.Bash` is a pure
# domain record of an intent ; the shell that runs is an adapter on the far
# side of this port.
#
# Vendored addition, not (yet) upstream hecks. WHY IT EXISTS: the
# corpus's tools.hecksagon wired its impure edges with the retired Rust
# runtime's `adapter :claude_tool, command: ..., tool: ...` form.
# HecksagonBuilder#adapter parses that form into `@raw_adapters` and #build
# never passes @raw_adapters into IR::Hecksagon — so every one of those
# bindings was silently discarded at boot. The door dispatched, mutated,
# emitted BashRan, and executed NOTHING. Proven live 2026-08-12: a dispatch
# of `echo ... > /tmp/probe` reported ✓ and the file did not exist.
#
# The fix is not to resurrect the Rust hook shape, but to say the same thing
# in hecks's own idiom — a `.port` with a verb, `.adapter`s declaring
# that port, and per-aggregate binds in the hecksagon:
#
#     Tools::ShellTool.executed_by("Shell")
#     Tools::FileTool.executed_by("Filesystem")
#     Tools::SearchTool.executed_by("Search")
#
# Those are ordinary `IR::Bind`s (BindingProxy#method_missing mints one for
# any verb), so Registry::Verification#check_verb type-checks them against
# this port at boot exactly as it does `persisted_by` — a bind naming an
# adapter that does not implement `executed_by` fails loudly at wiring
# rather than silently doing nothing, which is the whole failure this port
# is here to make impossible.
#
# SIGNAL IS :reply, deliberately. The effect-family shape the Pizzas example
# documents (`charged_by("Stripe", on: "OrderPlaced") do success ... end`)
# has no implementation anywhere in hecks — HecksagonBuilder#success /
# #failure are no-op stubs, and its own comment calls that "a whole
# subsystem, not a missing keyword". A tool call is synchronous and returns
# a value to the caller who asked for it, so :reply is also the honest
# signal here, not a workaround.
#
# RESOLUTION IS PER-AGGREGATE (binds_for), never a global scan of everything
# implementing the port — Ports::Extraction#adapter refuses to choose when
# more than one adapter implements a port, and three tool adapters share
# this one. Modelled on Ports::Projection, which has the same shape.

require_relative "../runtime/errors"

module Hecks
  module Ports
    module Execution
      NAME = "execution"
      VERB = "executed_by"

      # The shape every execution adapter returns, and the shape
      # Cascade.RecordResult's own attributes already expect:
      #   { tool: String, output: String, exit_code: Integer, ok: Boolean }
      module_function

      def binds_for(registry, domain, aggregate)
        registry.hecksagon(domain)&.binds_for(aggregate.hecks_name, VERB) || []
      end

      def bound?(registry, domain, aggregate) = !binds_for(registry, domain, aggregate).empty?

      # Run the bound adapter for this aggregate, or return nil when the
      # aggregate binds no execution adapter at all — the overwhelming
      # majority of aggregates, which are pure domain records with no impure
      # edge and must stay exactly that.
      def perform(registry, domain, aggregate, operation, args)
        bind = binds_for(registry, domain, aggregate).first
        return unless bind

        adapter_for(bind).execute(operation.to_s, plain(args))
      end

      def adapter_for(bind)
        Adapters.const_get(bind.adapter)
      rescue NameError
        raise Runtime::WiringError,
              "no Ruby adapter implementation for #{bind.adapter.inspect} " \
              "(expected Hecks::Adapters::#{bind.adapter})"
      end

      # THE CALLING CONVENTION LIVES HERE, not in each adapter. An adapter
      # receives a plain Symbol => String/Integer hash and never has to know
      # whether the dispatcher handed it raw kwargs or coerced Value objects
      # — one place to be right about, three adapters that stay simple.
      def plain(args)
        (args || {}).each_with_object({}) do |(key, value), out|
          out[key.to_sym] = unwrap(value)
        end
      end

      def unwrap(value)
        return value if value.is_a?(String) || value.is_a?(Integer) ||
                        value.is_a?(TrueClass) || value.is_a?(FalseClass) || value.nil?

        # A coerced Runtime::Value answers `.value` directly — the common
        # case once a command's arguments have passed through
        # CommandInterpreter. But `perform_execution` is called with the
        # DISPATCH's own raw kwargs (Dispatcher#dispatch never rebinds
        # `args` to the coerced form), so a single-field value-object
        # attribute just as often arrives here as the plain
        # `{ value: "..." }` shorthand the DSL itself accepts at the call
        # site — unwrapped the same way, one key deep, before falling
        # through to `.value`/`.to_s`. A genuinely multi-field hash (no
        # bare `:value` key) is passed through field by field rather than
        # collapsed to a single scalar, so an adapter reading more than
        # one of its own fields still can.
        if value.is_a?(Hash)
          return unwrap(value[:value]) if value.size == 1 && value.key?(:value)

          return value.transform_values { |v| unwrap(v) }
        end

        return unwrap(value.value) if value.respond_to?(:value)

        value.to_s
      end
    end
  end
end
