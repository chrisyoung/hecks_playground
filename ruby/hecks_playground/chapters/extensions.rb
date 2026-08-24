# = HecksPlayground::Chapters::Extensions
#
# Self-describing chapter for HecksPlayground extension infrastructure. Covers
# all pluggable extensions: HTTP serving, persistence, auth, ACL,
# web explorer, tenancy, metrics, PII, and more.
#
#   domain = HecksPlayground::Chapters::Extensions.definition
#   domain.aggregates.map(&:name)
#
module HecksPlayground
  module Chapters
    require_paragraphs(__FILE__)
    # HecksPlayground::Chapters::Extensions
    #
    # Bluebook chapter defining all pluggable HecksPlayground extensions: HTTP serving, persistence, auth, tenancy, and more.
    #
    module Extensions
      def self.definition
        @definition ||= Chapters.definition_from_bluebook("extensions")
      end
    end
  end
end
