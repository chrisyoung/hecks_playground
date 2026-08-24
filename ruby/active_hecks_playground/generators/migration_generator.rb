# ActiveHecks::MigrationGenerator
#
# Rails generator that produces SQL migration files from domain changes.
# Compares the current domain against the saved snapshot using DomainDiff
# and writes incremental SQL to db/hecks_playground_migrate/ (separate from ActiveRecord).
#
#   rails generate active_hecks_playground:migration
#
require "rails/generators"

module ActiveHecks
  class MigrationGenerator < ::Rails::Generators::Base
    desc "Generate HecksPlayground SQL migrations from domain changes"

    def generate_migration
      config = ::HecksPlayground.configuration
      unless config&.domain_obj
        say "HecksPlayground is not configured. Add HecksPlayground.configure in an initializer.", :red
        return
      end

      domain = config.domain_obj
      snapshot_path = ::Rails.root.join(HecksPlayground::Migrations::DomainSnapshot::DEFAULT_PATH).to_s
      old_domain = HecksPlayground::Migrations::DomainSnapshot.load(path: snapshot_path)

      changes = HecksPlayground::Migrations::DomainDiff.call(old_domain, domain)

      if changes.empty?
        say "Domain is up to date — no migrations needed.", :green
        return
      end

      unless HecksPlayground::Migrations::MigrationStrategy.for(:sql)
        HecksPlayground::Migrations::MigrationStrategy.register(:sql, HecksPlayground::Migrations::Strategies::SqlStrategy)
      end

      files = HecksPlayground::Migrations::MigrationStrategy.run_all(changes, output_dir: ::Rails.root.to_s)

      files.each { |f| say "Generated #{f}", :green }

      HecksPlayground::Migrations::DomainSnapshot.save(domain, path: snapshot_path)
      say "Saved domain snapshot to #{snapshot_path}", :green
    end
  end
end
