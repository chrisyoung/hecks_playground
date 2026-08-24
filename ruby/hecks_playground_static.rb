# = HecksStatic
#
# Standalone domain generator for HecksPlayground. Loaded from the Targets::Ruby
# Bluebook chapter — the chapter lists every aggregate, and
# load_aggregates derives the require tree from naming conventions.
#
# == Usage
#
#   require "hecks_playground_static"
#   domain = HecksPlayground.domain("Pizzas") { ... }
#   HecksStatic::GemGenerator.new(domain).generate
#
#   # Or via CLI:
#   hecks_playground build --standalone
#
HecksPlayground::Chapters.load_aggregates(
  HecksPlayground::Targets::Ruby,
  base_dir: File.expand_path("hecks_playground_static", __dir__)
)

# Self-register static Ruby target when loaded
HecksPlayground.register_target(:static) do |domain, version: "0.1.0", output_dir: ".", smoke_test: true, **|
  valid, errors = HecksPlayground.validate(domain)
  raise HecksPlayground::ValidationError.for_domain(errors) unless valid

  root = HecksStatic::GemGenerator.new(domain, version: version, output_dir: output_dir).generate
  HecksPlayground.send(:run_ruby_smoke_test, root, domain) if smoke_test
  root
end
