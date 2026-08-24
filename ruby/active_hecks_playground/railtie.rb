# ActiveHecks::Railtie
#
# Rails integration hook. Boots the HecksPlayground application container after
# initializers load, and provides rake tasks for migration generation
# and execution. All Rails-specific behavior lives here under ActiveHecks,
# keeping the core HecksPlayground namespace free of Rails concerns.
#
# Loaded automatically when Rails is detected:
#
#   require "active_hecks_playground/railtie" if defined?(::Rails::Railtie)
#
# Rake tasks:
#   rake hecks_playground:generate:migrations  — generate SQL from domain changes
#   rake hecks_playground:db:migrate           — run pending HecksPlayground SQL migrations
#
module ActiveHecks
  class Railtie < ::Rails::Railtie
    generators do
      require "active_hecks_playground/generators/init_generator"
      require "active_hecks_playground/generators/migration_generator"
    end

    initializer "active_hecks_playground.setup", after: :load_config_initializers do
      if HecksPlayground.configuration
        HecksPlayground.configuration.boot!
      end
    end

    rake_tasks do
      namespace :hecks_playground do
        desc "Generate HecksPlayground SQL migrations from domain changes"
        task "generate:migrations" => :environment do
          config = HecksPlayground.configuration
          unless config&.domain_obj
            puts "HecksPlayground is not configured."
            next
          end

          domain = config.domain_obj
          snapshot_path = ::Rails.root.join(HecksPlayground::Migrations::DomainSnapshot::DEFAULT_PATH).to_s
          old_domain = HecksPlayground::Migrations::DomainSnapshot.load(path: snapshot_path)
          changes = HecksPlayground::Migrations::DomainDiff.call(old_domain, domain)

          if changes.empty?
            puts "Domain is up to date — no migrations needed."
            next
          end

          unless HecksPlayground::Migrations::MigrationStrategy.for(:sql)
            HecksPlayground::Migrations::MigrationStrategy.register(:sql, HecksPlayground::Migrations::Strategies::SqlStrategy)
          end
          files = HecksPlayground::Migrations::MigrationStrategy.run_all(changes, output_dir: ::Rails.root.to_s)
          files.each { |f| puts "Generated #{f}" }

          HecksPlayground::Migrations::DomainSnapshot.save(domain, path: snapshot_path)
          puts "Saved domain snapshot to #{snapshot_path}"
        end

        desc "Run pending HecksPlayground SQL migrations"
        task "db:migrate" => :environment do
          connection = hecks_playground_connection
          runner = HecksPlayground::Migrations::MigrationRunner.new(
            connection: connection,
            migration_dir: ::Rails.root.join("db/hecks_playground_migrate").to_s
          )
          applied = runner.run_all

          if applied.empty?
            puts "No pending migrations."
          else
            applied.each { |f| puts "Applied #{f}" }
          end
        end
      end

      def hecks_playground_connection
        config = HecksPlayground.configuration
        db = config&.instance_variable_get(:@db)
        return db if db # Sequel::Database — responds to #execute and #transaction

        ActiveRecord::Base.connection
      end
    end
  end
end
