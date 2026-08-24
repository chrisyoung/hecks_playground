# Dev tools concern — development-time web capabilities
require_relative "dsl"

HecksPlayground.concern :dev_tools do
  includes :tailwind, :web_client_state, :web_debug
end
