# HecksPlayground::MultiDomain::QueueWiring
#
# Wires a MemoryQueue with a cross-domain command resolver.
#
#   HecksPlayground::MultiDomain::QueueWiring.wire(domains, runtimes)
#
module HecksPlayground
  module MultiDomain
    # HecksPlayground::MultiDomain::QueueWiring
    #
    # Wires a MemoryQueue with a cross-domain command resolver for routing commands across domain boundaries.
    #
    module QueueWiring
      extend HecksTemplating::NamingHelpers
      module_function

      def wire(domains, runtimes)
        HecksPlayground.queue = Queue::MemoryQueue.new(
          command_resolver: ->(cmd_name, attrs) {
            runtimes.each do |rt|
              rt.domain.aggregates.each do |agg|
                next unless agg.commands.any? { |c| c.name == cmd_name }
                mod = Object.const_get(domain_module_name(rt.domain.name))
                klass = HecksPlayground::Conventions::Names.resolve_command_const(mod, agg.name, cmd_name)
                return klass.call(**attrs)
              end
            end
            raise HecksPlayground::Error, "Command not found: #{cmd_name}"
          }
        )
      end
    end
  end
end
