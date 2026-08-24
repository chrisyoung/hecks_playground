# = HecksPersist
#
# SQL persistence layer for HecksPlayground domains. Provides Sequel-based repository
# adapters, SQL schema generation, and migration file generation. Supports
# SQLite, PostgreSQL, and MySQL via the Sequel gem.
#
# This module registers autoloads for three generator/strategy classes:
#
# - {HecksPlayground::Generators::SQL::SqlAdapterGenerator} -- Generates Sequel-based
#   repository adapter classes for each aggregate
# - {HecksPlayground::Generators::SQL::SqlBuilder} -- Builds SQL CREATE TABLE statements
#   from domain attribute definitions
# - {HecksPlayground::Generators::SQL::SqlMigrationGenerator} -- Generates Sequel
#   migration files from domain diffs
# - {HecksPlayground::Migrations::Strategies::SqlStrategy} -- Executes SQL migrations
#   against a live database connection
#
# == Usage
#
#   require "hecks_playground_persist"
#   app = HecksPlayground.boot(__dir__, adapter: :sqlite)
#
# This is a separate entry point (future gem: +hecks_playground_persist+) to keep
# Sequel and database dependencies isolated from the core framework.
#
module HecksPlayground
  module Generators
    # = HecksPlayground::Generators::SQL
    #
    # Namespace for SQL-related code generators. Contains the adapter
    # generator (produces repository classes), the SQL builder (produces
    # DDL statements), and the migration generator (produces versioned
    # migration files).
    module SQL
      autoload :SqlAdapterGenerator,   "hecks_playground_persist/sql_adapter_generator"
      autoload :SqlBuilder,            "hecks_playground_persist/sql_builder"
      autoload :SqlMigrationGenerator, "hecks_playground_persist/sql_migration_generator"
    end
  end

  module Boot
    autoload :SqlBoot,            "hecks_playground_persist/sql_boot"
    autoload :DatabaseConnection, "hecks_playground_persist/database_connection"
  end

  module Migrations
    module Strategies
      # = HecksPlayground::Migrations::Strategies::SqlStrategy
      #
      # Strategy for applying domain migrations to a SQL database via Sequel.
      # Reads migration files from +db/hecks_playground_migrate/+ and executes them
      # in order against the configured database connection.
      autoload :SqlStrategy, "hecks_playground_persist/sql_strategy"
      autoload :SqlHelpers, "hecks_playground_persist/sql_helpers"
    end
  end
end
