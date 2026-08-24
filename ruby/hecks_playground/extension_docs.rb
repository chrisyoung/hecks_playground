# HecksPlayground::ExtensionDocs
#
# Metadata registry for extension gems. Each entry describes a gem that
# extends HecksPlayground with an external integration (database, server, AI, etc.).
# Used by ReadmeGenerator to produce the {{connections}} section.
#
#   HecksPlayground::ExtensionDocs.all
#   # => [{ gem: "hecks_playground_sqlite", name: "SQLite", ... }, ...]
#
module HecksPlayground
  # HecksPlayground::ExtensionDocs
  #
  # Metadata registry for extension gems used by ReadmeGenerator to produce the connections section.
  #
  module ExtensionDocs
    EXTENSIONS = [
      {
        gem: "hecks_playground_sqlite",
        name: "SQLite",
        description: "SQLite persistence — zero-config, file-based SQL database",
        gemfile: 'gem "hecks_playground_sqlite"',
        example: "# Just add the gem. SQLite auto-wires on boot.\nCat.create(name: \"Whiskers\")\nCat.all  # persisted to SQLite"
      },
      {
        gem: "hecks_playground_postgres",
        name: "PostgreSQL",
        description: "PostgreSQL persistence — production-grade relational database",
        gemfile: 'gem "hecks_playground_postgres"',
        example: "# Set HECKS_PLAYGROUND_DB_HOST, HECKS_PLAYGROUND_DB_NAME, HECKS_PLAYGROUND_DB_USER env vars\n# or configure in boot block:\nCatsDomain.boot(adapter: { type: :postgres, host: \"localhost\", database: \"cats\" })"
      },
      {
        gem: "hecks_playground_mysql",
        name: "MySQL",
        description: "MySQL persistence — widely deployed relational database",
        gemfile: 'gem "hecks_playground_mysql"',
        example: "# Set HECKS_PLAYGROUND_DB_HOST, HECKS_PLAYGROUND_DB_NAME, HECKS_PLAYGROUND_DB_USER env vars"
      },
      {
        gem: "hecks_playground_serve",
        name: "HTTP Server",
        description: "REST and JSON-RPC server — serve your domain over HTTP",
        gemfile: 'gem "hecks_playground_serve"',
        example: "CatsDomain.serve(port: 9292)"
      },
      {
        gem: "hecks_playground_ai",
        name: "MCP Server",
        description: "Model Context Protocol — expose your domain to AI agents",
        gemfile: 'gem "hecks_playground_ai"',
        example: "CatsDomain.mcp"
      },
      {
        gem: "hecks_playground_auth",
        name: "Auth",
        description: "Authentication & authorization — actor-based access control on commands",
        gemfile: 'gem "hecks_playground_auth"',
        example: "# DSL: actor \"Admin\" on commands\n# App: HecksPlayground.actor = current_user\n# Auth middleware checks role automatically"
      },
      {
        gem: "hecks_playground_pii",
        name: "PII Protection",
        description: "Mark attributes as PII — masking, redaction, and GDPR erasure",
        gemfile: 'gem "hecks_playground_pii"',
        example: "# DSL: attribute :email, String, pii: true\n# Erasure: CatsDomain.erase_pii(customer_id)\n# Introspection: CatsDomain.pii_fields"
      },
      {
        gem: "hecks_playground_tenancy",
        name: "Multi-tenancy",
        description: "Tenant isolation — same domain, different data per tenant",
        gemfile: 'gem "hecks_playground_tenancy"',
        example: "# Declare in DSL: tenancy :column\nHecks.tenant = \"acme\"\nCat.all  # only acme's cats"
      },
    ].freeze

    def self.all
      EXTENSIONS
    end
  end
end
