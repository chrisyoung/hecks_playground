# = GoHecks
#
# Go domain generator for HecksPlayground. Loaded from the Targets::Go
# Bluebook chapter — the chapter lists every aggregate, and
# load_aggregates derives the require tree from naming conventions.
#
# == Usage
#
#   require "go_hecks_playground"
#   domain = HecksPlayground.domain("Pizzas") { ... }
#   GoHecks::ProjectGenerator.new(domain).generate
#
#   # Or via CLI:
#   hecks_playground build --target go
#
HecksPlayground::Chapters.load_aggregates(
  HecksPlayground::Targets::Go,
  base_dir: File.expand_path("go_hecks_playground", __dir__)
)

# Self-register Go targets when loaded
HecksPlayground.register_target(:go) do |domain, output_dir: ".", smoke_test: true, **|
  generator = GoHecks::ProjectGenerator.new(domain, output_dir: output_dir)
  root = generator.generate
  if smoke_test
    HecksPlayground.send(:run_smoke_test, root, domain) rescue nil
  end
  root
end

HecksPlayground.register_target(:binary) do |domain, output_dir: "bin", **|
  GoHecks::BinaryBuilder.build(domain, output_dir: output_dir)
end
