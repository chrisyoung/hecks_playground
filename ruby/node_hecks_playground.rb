# = NodeHecks
#
# Node.js/TypeScript domain generator for HecksPlayground. Loaded from the
# Targets::Node Bluebook chapter — the chapter lists every aggregate,
# and load_aggregates derives the require tree from naming conventions.
#
# == Usage
#
#   require "node_hecks_playground"
#   domain = HecksPlayground.domain("Pizzas") { ... }
#   NodeHecks::ProjectGenerator.new(domain).generate
#
#   # Or via CLI:
#   hecks_playground build --target node
#

# Register Node.js/TypeScript type mappings with the TypeContract registry
HecksPlayground::Conventions::TypeContract.register_target(:node, {
  "String"   => "string",
  "Integer"  => "number",
  "Float"    => "number",
  "Boolean"  => "boolean",
  "TrueClass" => "boolean",
  "FalseClass" => "boolean",
  "Date"     => "string",
  "DateTime" => "string",
  "JSON"     => "Record<string, unknown>",
}, default: "string")

HecksPlayground::Chapters.load_aggregates(
  HecksPlayground::Targets::Node,
  base_dir: File.expand_path("node_hecks_playground", __dir__)
)

# Self-register Node target when loaded
HecksPlayground.register_target(:node) do |domain, output_dir: ".", **|
  valid, errors = HecksPlayground.validate(domain)
  raise HecksPlayground::ValidationError.for_domain(errors) unless valid

  NodeHecks::ProjectGenerator.new(domain, output_dir: output_dir).generate
end
