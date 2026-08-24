# HecksPlayground::Concerns::DSL
#
# DSL for declaring composite capabilities (concerns). A concern
# is a named list of abilities that can be applied together.
#
#   # hecks_playground/concerns/webapp.rb
#   HecksPlayground.concern :webapp do
#     includes :static_assets, :websocket, :live_reload, :client_commands, :readme
#   end
#
module HecksPlayground
  module Concerns
    # HecksPlayground::Concerns::DSL
    #
    # Parses concern declarations and registers them as composite capabilities.
    #
    class DSL
      attr_reader :name

      def initialize(name)
        @name = name.to_sym
        @bundled = []
      end

      def includes(*names)
        @bundled.concat(names.map(&:to_sym))
      end

      def register!
        bundled = @bundled.freeze
        HecksPlayground.register_capability(@name) do |runtime|
          excluded = HecksPlayground.instance_variable_get(:@_excluded_capabilities) || []
          bundled.each do |cap|
            next if excluded.include?(cap)
            begin; require "hecks_playground/capabilities/#{cap}"; rescue LoadError; end
            hook = HecksPlayground.capability_registry[cap]
            hook.call(runtime) if hook
          end
        end
      end
    end
  end

  # Top-level DSL entry point.
  #
  #   HecksPlayground.concern :webapp do
  #     includes :static_assets, :websocket
  #   end
  def self.concern(name, &block)
    dsl = Concerns::DSL.new(name)
    dsl.instance_eval(&block)
    dsl.register!
  end
end
