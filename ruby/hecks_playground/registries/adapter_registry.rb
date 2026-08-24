# HecksPlayground::AdapterRegistryMethods
#
# Registry for persistence adapter types. Each adapter (memory, sqlite, etc.)
# registers itself so that BluebookConnections and Boot can check adapter
# availability without hardcoded lists.
#
#   HecksPlayground.register_adapter(:sqlite)
#   HecksPlayground.registered_adapters  # => [:memory, :sqlite, ...]
#   HecksPlayground.adapter?(:sqlite)    # => true
#
module HecksPlayground
  # HecksPlayground::AdapterRegistryMethods
  #
  # Registry for persistence adapter types (memory, sqlite, etc.) extended onto the HecksPlayground module.
  #
  module AdapterRegistryMethods
    def registered_adapters
      adapter_registry.all
    end

    def register_adapter(name)
      adapter_registry.register(name)
    end

    def adapter?(name)
      adapter_registry.include?(name)
    end

    private

    def adapter_registry
      @adapter_registry ||= SetRegistry.new(%i[memory sqlite postgres mysql mysql2 filesystem filesystem_store mongodb information])
    end
  end
end
