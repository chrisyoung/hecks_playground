# = BlueBook
#
# The domain command language for HecksPlayground. Named for Evans' DDD Blue Book
# and Smalltalk's Blue Book — the two traditions this grammar descends from.
#
# Loaded from its own Bluebook chapter — the chapter lists every aggregate,
# and load_aggregates derives the require tree from naming conventions.
# The Bluebook IS the Bluebook.
#
#   ast = BlueBook::Grammar.parse("Pizza.attr :name, String")
#

# Bootstrap: Tokenizer, IR, DSL kernel — the minimum to run BluebookBuilder.
# These must load before any chapter can describe aggregates, so they
# cannot use chapter-driven loading. They define the types and builders
# that BluebookBuilder.new("tmp") needs inside every chapter .define method.
require "bluebook/tokenizer"
require "hecks_playground/bluebook_model/behavior"
require "hecks_playground/bluebook_model/structure"
require "hecks_playground/bluebook_model/names"
require "hecks_playground/dsl/describable"
require "hecks_playground/dsl/type_name"
require "hecks_playground/dsl/attribute_collector"
require "hecks_playground/dsl/event_builder"
require "hecks_playground/dsl/command_builder"
require "hecks_playground/dsl/value_object_builder"
require "hecks_playground/dsl/entity_builder"
require "hecks_playground/dsl/policy_builder"
require "hecks_playground/dsl/lifecycle_builder"
require "hecks_playground/dsl/read_model_builder"
require "hecks_playground/dsl/service_builder"
require "hecks_playground/dsl/workflow_builder"
require "hecks_playground/dsl/aggregate_builder"
require "hecks_playground/dsl/aggregate_rebuilder"
require "hecks_playground/dsl/bluebook_builder"
require "hecks_playground/dsl/bluebook_builder"

# Chapter infrastructure (must load before any chapter files)
require "hecks_playground/chapters"

# Load the Bluebook chapter (paragraphs describe all aggregates)
require "hecks_playground/chapters/bluebook"

# Implementation files are loaded by HecksPlayground::Chapters.load_chapter
# after the full framework infrastructure is available.
# See hecksties/lib/hecks_playground.rb for the boot sequence.

module BlueBook
  VERSION = "2026.03.29.1"

  def self.register!
    HecksPlayground.register_grammar(:bluebook) do |g|
      g.parser = BlueBook::Grammar
      g.builder = HecksPlayground::DSL::BluebookBuilder
      g.entry_point = :domain
      g.bare_commands = BlueBook::Grammar::BARE_COMMANDS
      g.handle_methods = BlueBook::Grammar::HANDLE_METHODS
      g.type_map = BlueBook::Grammar::TYPE_MAP
    end
  end
end
