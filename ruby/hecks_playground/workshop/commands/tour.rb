# HecksPlayground::CLI tour command
#
# Launches a guided walkthrough. Without flags, runs the domain modeler's
# sketch -> play -> build loop. With --architecture, runs a contributor's
# walkthrough of the framework internals.
#
#   hecks_playground tour
#   hecks_playground tour --architecture
#
HecksPlayground::CLI.register_command(
  :tour,
  "Guided walkthrough of the workshop (--architecture for internals)",
  options: { architecture: { type: :boolean, default: false, desc: "Contributor walkthrough of framework components" } }
) do
  if options[:architecture]
    HecksPlayground::ArchitectureTour.new.start
  else
    HecksPlayground::Workshop::WorkshopRunner.new.tour
  end
end
