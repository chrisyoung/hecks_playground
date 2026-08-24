HecksPlayground::Chapters.load_aggregates(
  HecksPlayground::Bluebook::ToolingParagraph,
  base_dir: File.expand_path("docs", __dir__)
)
  # HecksPlayground::ExtensionDocs
  #
  # Metadata registry for all HecksPlayground extensions. Each entry describes what an
  # extension does, which gem provides it, how to install it, and includes a
  # usage example. This registry serves two purposes:
  #
  # 1. Provides structured data used by ReadmeGenerator for the main project
  #    README and for per-extension documentation files.
  # 2. Offers programmatic access to extension metadata for tooling,
  #    introspection, and documentation generation.
  #
  # Extensions are grouped by category: +:persistence+, +:realtime+, +:rails+,
  # +:server+, and +:middleware+.
  #
  #   HecksPlayground::ExtensionDocs.all                    # => Array of all extension hashes
  #   HecksPlayground::ExtensionDocs.by_category            # => Hash grouped by category symbol
  #   HecksPlayground::ExtensionDocs.generate_readmes(root) # => generates docs/extensions/*.md
  #

module HecksPlayground
  module ExtensionDocs
    # @return [Array<Hash>] frozen array of extension metadata hashes. Each hash
    #   contains keys:
    #   - +:gem+ [String] - the gem name (e.g. "hecks_playground_sqlite")
    #   - +:name+ [String] - human-readable display name (e.g. "SQLite")
    #   - +:category+ [Symbol] - grouping category (:persistence, :middleware, etc.)
    #   - +:description+ [String] - one-line description of the extension
    #   - +:gemfile+ [String] - the Gemfile line to install the gem
    #   - +:example+ [String] - a Ruby code example showing usage
    EXTENSIONS = [
      {
        gem: "hecks_playground_sqlite",
        name: "SQLite",
        category: :persistence,
        description: "SQLite persistence — zero-config, file-based SQL database",
        gemfile: 'gem "hecks_playground_sqlite"',
        example: "# Just add the gem. SQLite auto-wires on boot.\nCat.create(name: \"Whiskers\")\nCat.all  # persisted to SQLite"
      },
      {
        gem: "hecks_playground_postgres",
        name: "PostgreSQL",
        category: :persistence,
        description: "PostgreSQL persistence — production-grade relational database",
        gemfile: 'gem "hecks_playground_postgres"',
        example: "# Set HECKS_PLAYGROUND_DB_HOST, HECKS_PLAYGROUND_DB_NAME, HECKS_PLAYGROUND_DB_USER\nCatsDomain.boot(adapter: { type: :postgres, host: \"localhost\", database: \"cats\" })"
      },
      {
        gem: "hecks_playground_mysql",
        name: "MySQL",
        category: :persistence,
        description: "MySQL persistence — widely deployed relational database",
        gemfile: 'gem "hecks_playground_mysql"',
        example: "# Set HECKS_PLAYGROUND_DB_HOST, HECKS_PLAYGROUND_DB_NAME, HECKS_PLAYGROUND_DB_USER env vars"
      },
      {
        gem: "hecks_playground_live",
        name: "Live Events",
        category: :realtime,
        description: "Real-time domain event streaming via Turbo Streams + ActionCable",
        gemfile: 'gem "hecks_playground_live"',
        example: "# Rails view:\n<%= turbo_stream_from \"hecks_playground_live_events\" %>\n<div id=\"event-feed\"></div>\n# Events auto-prepend. No custom JS."
      },
      {
        gem: "hecks_playground_on_rails",
        name: "Rails (Full)",
        category: :rails,
        description: "Bundles ActiveHecks + HecksLive — one gem for Rails apps",
        gemfile: 'gem "hecks_playground_on_rails"',
        example: "rails generate active_hecks_playground:init\n# That's it. Everything wires up."
      },
      {
        gem: "hecks_playground_serve",
        name: "HTTP Server",
        category: :server,
        description: "REST and JSON-RPC server with OpenAPI and JSON Schema generation",
        gemfile: 'gem "hecks_playground_serve"',
        example: "CatsDomain.serve(port: 9292)"
      },
      {
        gem: "hecks_playground_ai",
        name: "MCP Server",
        category: :server,
        description: "Model Context Protocol — expose your domain to AI agents",
        gemfile: 'gem "hecks_playground_ai"',
        example: "CatsDomain.mcp"
      },
      {
        gem: "hecks_playground_auth",
        name: "Auth",
        category: :middleware,
        description: "Actor-based access control on commands",
        gemfile: 'gem "hecks_playground_auth"',
        example: "# DSL: actor \"Admin\" on commands\nHecks.actor = current_user\n# Middleware checks role automatically"
      },
      {
        gem: "hecks_playground_audit",
        name: "Audit Trail",
        category: :middleware,
        description: "Immutable event log with command context, actor, and tenant",
        gemfile: 'gem "hecks_playground_audit"',
        example: "# Auto-records every domain event.\n# Pairs with hecks_playground_auth for actor tracking."
      },
      {
        gem: "hecks_playground_pii",
        name: "PII Protection",
        category: :middleware,
        description: "Mark attributes as PII — masking, redaction, and GDPR erasure",
        gemfile: 'gem "hecks_playground_pii"',
        example: "# DSL: attribute :email, String, pii: true\nCatsDomain.erase_pii(customer_id)"
      },
      {
        gem: "hecks_playground_tenancy",
        name: "Multi-tenancy",
        category: :middleware,
        description: "Tenant isolation — same domain, different data per tenant",
        gemfile: 'gem "hecks_playground_tenancy"',
        example: "# DSL: tenancy :column\nHecks.tenant = \"acme\"\nCat.all  # only acme's cats"
      },
      {
        gem: "hecks_playground_idempotency",
        name: "Idempotency",
        category: :middleware,
        description: "Command deduplication by fingerprinting within a TTL window",
        gemfile: 'gem "hecks_playground_idempotency"',
        example: "# Same command re-executed within TTL returns cached result.\n# HECKS_PLAYGROUND_IDEMPOTENCY_TTL=300"
      },
      {
        gem: "hecks_playground_rate_limit",
        name: "Rate Limiting",
        category: :middleware,
        description: "Sliding window rate limiting per actor",
        gemfile: 'gem "hecks_playground_rate_limit"',
        example: "# HECKS_PLAYGROUND_RATE_LIMIT=60  (max commands per window)\n# HECKS_PLAYGROUND_RATE_PERIOD=60 (window in seconds)"
      },
      {
        gem: "hecks_playground_retry",
        name: "Retry",
        category: :middleware,
        description: "Auto-retry failed commands with exponential backoff",
        gemfile: 'gem "hecks_playground_retry"',
        example: "# Only retries transient errors, not domain errors.\n# HECKS_PLAYGROUND_RETRY_MAX=3\n# HECKS_PLAYGROUND_RETRY_DELAY=0.1"
      },
      {
        gem: "hecks_playground_transactions",
        name: "Transactions",
        category: :middleware,
        description: "Wraps command execution in database transactions when SQL is present",
        gemfile: 'gem "hecks_playground_transactions"',
        example: "# Auto-detects Sequel repositories.\n# Falls through for memory adapters."
      },
      {
        gem: "hecks_playground_logging",
        name: "Logging",
        category: :middleware,
        description: "Structured command logging — name, duration, actor, tenant",
        gemfile: 'gem "hecks_playground_logging"',
        example: "# Output:\n# [hecks_playground] CreatePizza 0.3ms actor=admin tenant=acme"
      },
      {
        gem: "hecks_playground_cqrs",
        name: "CQRS",
        category: :persistence,
        description: "Named persistence connections for read/write separation",
        gemfile: 'gem "hecks_playground_cqrs"',
        example: "CatsDomain.boot do\n  persist_to :write, :sqlite\n  persist_to :read, :sqlite, database: \"read.db\"\nend"
      },
    ].freeze

    # Return the full list of extension metadata hashes.
    #
    # @return [Array<Hash>] all registered extension metadata entries
    def self.all
      EXTENSIONS
    end

    # Return extensions grouped by their category symbol.
    #
    # @return [Hash{Symbol => Array<Hash>}] a hash where keys are category
    #   symbols (e.g. +:persistence+, +:middleware+) and values are arrays
    #   of extension metadata hashes belonging to that category
    def self.by_category
      EXTENSIONS.group_by { |e| e[:category] }
    end

    # Generate a Markdown README file for each extension that has a source
    # file present under +root/lib/+. Delegates to ReadmeWriter.
    #
    # @param root [String] the project root directory path
    # @return [Array<String>] paths to all generated README files
    def self.generate_readmes(root)
      ReadmeWriter.generate(root)
    end
  end
end
