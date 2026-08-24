# HecksPlayground::CLI allure command
#
# Launches the Allure terminal UI — a panel-based view of any domain.
# Boots the domain from a .bluebook file, expresses each concept as a
# panel using DomainPresentation, and renders them in the terminal.
#
#   hecks_playground allure                                  # show all nursery domains
#   hecks_playground allure pizzas.bluebook                  # view a specific domain
#   hecks_playground allure nursery/veterinary/veterinary.bluebook
#
HecksPlayground::CLI.handle(:allure) do |inv|
  bluebook_path = inv.args.first
  require "hecks_playground_cli/allure_renderer"

  if bluebook_path.nil?
    say "Usage: hecks_playground allure <bluebook-file>", :red
    exit 1
  end

  unless File.exist?(bluebook_path)
    say "Cannot read #{bluebook_path}", :red
    exit 1
  end

  renderer = HecksPlayground::CLI::AllureRenderer.new(bluebook_path)
  renderer.run
end
