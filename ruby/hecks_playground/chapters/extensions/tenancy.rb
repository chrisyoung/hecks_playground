# = HecksPlayground::Chapters::Extensions::TenancyChapter
#
# Self-describing sub-chapter for tenancy extension internals:
# ownership-scoped and tenant-scoped repository proxies.
#
#   HecksPlayground::Chapters::Extensions::TenancyChapter.define(builder)
#
module HecksPlayground
  module Chapters
    module Extensions
      # HecksPlayground::Chapters::Extensions::TenancyChapter
      #
      # Bluebook sub-chapter for tenancy internals: ownership-scoped and tenant-scoped repository proxies.
      #
      module TenancyChapter
        def self.define(b)
          b.aggregate "OwnershipScopedRepository", "Repository proxy filtering by ownership field identity" do
            command("ScopeToOwner") { attribute :owner_id, String }
            command("VerifyOwnership") { attribute :record_id, String }
          end

          b.aggregate "TenantScopedRepository", "Repository proxy maintaining separate instances per tenant" do
            command("ScopeToTenant") { attribute :tenant_id, String }
            command("ForTenant") { attribute :tenant_id, String }
          end
        end
      end
    end
  end
end
