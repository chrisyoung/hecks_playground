# Hecks::DSL::TypeName
#
# @domain Layout
#
# Predicate for detecting type names in the ubiquitous language.
# A type name starts with an uppercase letter (PascalCase).
#
#   TypeName.match?("Pizza")      # => true
#   TypeName.match?("name")       # => false
#   TypeName.match?(Pizza)        # => true (Module)
#   TypeName.match?(String)       # => true (Class)
#
module Hecks
  module DSL
    module TypeName
      PATTERN = /\A[A-Z]/

      def self.match?(value)
        value.to_s.match?(PATTERN)
      end

      # True when this stands where a FIELD NAME belongs but is a type —
      # `attribute ServiceName`, the attribute named by its value object.
      # A Symbol or String answers to to_sym and is never a bare constant.
      def self.bare_constant?(value)
        !value.nil? && !value.respond_to?(:to_sym) && match?(value)
      end

      # The field a bare constant names : `ServiceName` -> :service_name.
      # Mirrors the Rust parser, which dumps `attribute ServiceName` as
      # `service_name : ServiceName`.
      def self.field_name(value)
        Hecks::Utils.underscore(Hecks::Utils.const_short_name(value)).to_sym
      end
    end
  end
end
