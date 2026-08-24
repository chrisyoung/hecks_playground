# Webapp concern — web app runtime stack
require_relative "dsl"

HecksPlayground.concern :webapp do
  includes :project_discovery, :static_assets, :websocket, :live_reload, :client_commands, :readme
end
