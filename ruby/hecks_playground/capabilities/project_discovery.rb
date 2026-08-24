# HecksPlayground::Capabilities::ProjectDiscovery
#
# Discovers and boots HecksPlayground projects from the filesystem.
# Scans directories for hecks_playground/*.bluebook files, boots each
# project, and provides state serialization for connected clients.
#
#   HecksPlayground.hecksagon "MyIDE" do
#     capabilities :project_discovery
#   end
#
require_relative "dsl"
require_relative "project_discovery/bridge"
require_relative "project_discovery/ws_handler"

module HecksPlayground
  module Capabilities
    # HecksPlayground::Capabilities::ProjectDiscovery
    #
    # Filesystem project discovery and domain state serialization.
    #
    module ProjectDiscovery
      def self.apply(runtime)
        bridge = Bridge.new
        runtime.instance_variable_set(:@project_bridge, bridge)
        runtime.define_singleton_method(:projects) { @project_bridge }
        WsHandler.wire(runtime, bridge)
        bridge
      end
    end
  end
end

HecksPlayground.capability :project_discovery do
  description "Discover and boot HecksPlayground projects from the filesystem"
  direction :driving
  on_apply do |runtime|
    HecksPlayground::Capabilities::ProjectDiscovery.apply(runtime)
  end
end
