module HecksPlayground
  module Connections
    # HecksPlayground::Connections::McpConnection
    #
    # Wraps the MCP server as a +listens_to+ connection adapter. When a domain
    # declares +listens_to :mcp, transport: :stdio+, this connection boots a
    # DomainServer and exposes domain commands/queries as MCP tools over stdio.
    #
    # This is the adapter layer between the HecksPlayground connection system and the
    # MCP::DomainServer. It implements the connection interface expected by
    # the HecksPlayground runtime (+initialize+ with domain/runtime, +run+ to start).
    #
    #   listens_to :mcp, transport: :stdio
    #
    class McpConnection
      # Initializes the connection by creating a DomainServer for the given domain.
      #
      # @param domain [HecksPlayground::BluebookModel::Structure::Domain] the domain model to expose
      # @param runtime [HecksPlayground::Runtime] the runtime instance (unused, required by interface)
      def initialize(domain, runtime)
        @server = MCP::DomainServer.new(domain)
      end

      # Starts the MCP server over stdio transport. This method blocks until
      # the transport is closed.
      #
      # @return [void]
      def run
        @server.run
      end
    end
  end
end
