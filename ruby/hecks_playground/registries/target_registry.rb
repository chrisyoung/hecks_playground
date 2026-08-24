# HecksPlayground::TargetRegistryMethods
#
# Registry for build targets. Each target (ruby, static, go, rails) registers
# a callable that receives a domain and options, and returns the build output.
#
#   # Targets self-register when their gem is required:
#   require "go_hecks_playground"  # registers :go and :binary
#   HecksPlayground.target_registry[:go].call(domain)
#
module HecksPlayground
  # HecksPlayground::TargetRegistryMethods
  #
  # Registry for build targets (ruby, static, go, rails) extended onto the HecksPlayground module.
  #
  module TargetRegistryMethods
    def target_registry
      @target_registry ||= Registry.new
    end

    def register_target(name, &builder)
      target_registry.register(name, builder)
    end
  end
end
