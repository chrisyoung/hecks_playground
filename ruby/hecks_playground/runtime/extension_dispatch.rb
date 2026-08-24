# HecksPlayground::Runtime::ExtensionDispatch
#
# Applies extensions and capabilities to a live runtime. Extensions add
# infrastructure behavior; capabilities enrich the domain IR.
#
#   runtime.extend(:logging)
#   runtime.capability(:crud)
#
module HecksPlayground
  class Runtime
    # HecksPlayground::Runtime::ExtensionDispatch
    #
    # Applies extensions and capabilities to a live runtime without rebooting.
    #
    module ExtensionDispatch
      # Apply an extension to the live runtime without rebooting.
      #
      # @param name [Symbol] the registered extension name
      # @param kwargs [Hash] extension-specific options
      # @return [void]
      def extend(name, **kwargs)
        if HecksPlayground.extension_registry.empty?
          require "hecks_playground/runtime/load_extensions"
          HecksPlayground::LoadExtensions.require_all
        end
        hook = HecksPlayground.extension_registry[name.to_sym]
        raise "Unknown extension: #{name}. Available: #{HecksPlayground.extension_registry.keys.join(', ')}" unless hook
        if kwargs.any? && @mod.respond_to?(:connections)
          @mod.connections[:sends] << { name: name.to_sym, **kwargs }
        end
        hook.call(@mod, @domain, self, **kwargs)
        puts "  \e[32m✓\e[0m #{name}"
      end

      # Apply a capability to the live runtime, enriching the domain IR.
      #
      # @param name [Symbol] the registered capability name
      # @return [void]
      def capability(name)
        load_capability(name)
        hook = HecksPlayground.capability_registry[name.to_sym]
        raise "Unknown capability: #{name}. Available: #{HecksPlayground.capability_registry.keys.join(', ')}" unless hook
        hook.call(self)
      end

      def load_capability(name)
        require "hecks_playground/capabilities/#{name}"
      rescue LoadError
        require "hecks_playground/concerns/#{name}"
      end

      private

      # Apply capabilities declared in the Hecksagon file.
      # Stores exclusions so composite capabilities can check them.
      # Port-only declarations (driving/driven without a capability file) are skipped.
      def apply_hecksagon_capabilities
        return unless @hecksagon
        excluded = @hecksagon.excluded_capabilities || []
        HecksPlayground.instance_variable_set(:@_excluded_capabilities, excluded)

        # Apply concerns first (infrastructure: static_assets, websocket, etc.)
        (@hecksagon.concerns || []).each do |concern_name|
          next if excluded.include?(concern_name)
          begin
            load_capability(concern_name)
            hook = HecksPlayground.capability_registry[concern_name.to_sym]
            hook.call(self) if hook
          rescue LoadError, RuntimeError
          end
        end

        # Apply individual capabilities after concerns
        (@hecksagon.capabilities || []).each do |cap|
          next if excluded.include?(cap)
          begin
            capability(cap)
          rescue LoadError, RuntimeError
          end
        end

        HecksPlayground.instance_variable_set(:@_excluded_capabilities, [])
      end
    end
  end
end
