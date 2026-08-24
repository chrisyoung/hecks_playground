HecksPlayground::CLI.register_command(:console, "Start the interactive workshop",
  args: ["NAME"]
) do |name = nil|
  HecksPlayground::Workshop::WorkshopRunner.new(name: name).run
end
