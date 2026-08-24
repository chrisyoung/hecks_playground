# HecksPlayground::CLI appeal command
#
# Launches the HecksAppeal IDE server. Discovers hecks_playground projects in the
# given directory (or current directory) and opens the browser interface.
#
#   hecks_playground appeal                    # serve current directory
#   hecks_playground appeal /path/to/project   # serve specific project
#
HecksPlayground::CLI.handle(:appeal) do |inv|
  require "hecks_playground/appeal/server"
  HecksPlayground::Appeal::Server.run(inv.args)
end
