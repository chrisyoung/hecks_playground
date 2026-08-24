# HecksPlayground::Compiler
#
# Binary compiler for HecksPlayground v0. Concatenates all framework source
# files in load order into a single self-contained Ruby script.
# The compiled binary boots HecksPlayground with zero require_relative.
#
#   require "hecks_playground/compiler"
#   HecksPlayground::Compiler::BinaryCompiler.new.compile(output: "hecks_playground_v0")
#
module HecksPlayground
  module Compiler
  end
end

HecksPlayground::Chapters.load_aggregates(
  HecksPlayground::Bluebook::ToolingParagraph,
  base_dir: File.expand_path("compiler", __dir__)
)
