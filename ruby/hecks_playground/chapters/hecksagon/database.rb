# HecksPlayground::Chapters::Hecksagon::DatabaseParagraph
#
# Paragraph covering database connectivity and persistence extensions:
# connection pooling, MongoDB adapter/boot, and SQL dialect extensions.
#
#   HecksPlayground::Chapters::Hecksagon::DatabaseParagraph.define(builder)
#
module HecksPlayground
  module Chapters
    module Hecksagon
      module DatabaseParagraph
        def self.define(b)
          b.aggregate "DatabaseConnection" do
            description "Connects to databases via Sequel: MySQL, Postgres, SQLite, URLs"
            namespace "HecksPlayground::Boot"
            command("Connect") { attribute :url, String }
          end

          b.aggregate "MongoAdapterGenerator" do
            description "Generates MongoDB repository adapter classes for each aggregate"
            namespace "HecksPlayground::Generators::Mongo"
            inherits "HecksPlayground::Generator"
            command "Generate"
          end

          b.aggregate "MongoBoot" do
            description "MongoDB adapter lifecycle: connect, generate repos, return adapters"
            namespace "HecksPlayground::Boot"
            command "Setup"
          end

          b.aggregate "HecksMysql" do
            description "MySQL persistence extension, auto-wires SQL adapters via Sequel mysql2"
            command "Boot"
          end

          b.aggregate "HecksPostgres" do
            description "PostgreSQL persistence extension, auto-wires SQL adapters via Sequel pg"
            command "Boot"
          end

          b.aggregate "HecksSqlite" do
            description "SQLite persistence extension, auto-wires in-memory SQL adapters"
            command "Boot"
          end
        end
      end
    end
  end
end
