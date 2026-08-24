# HecksDeprecations::ConnectionConfigCompat
#
# Registers deprecated hash-style == on PersistConfig, SendConfig,
# and ExtensionConfig.
#
#   config == { type: :sqlite }  # => warns, then compares via to_h
#
require "hecks_playground/bluebook/connection_config"

# PersistConfig
HecksDeprecations.register(HecksPlayground::PersistConfig, :==) do |other|
  next super(other) unless other.is_a?(Hash)
  HecksDeprecations.warn_deprecated(self.class, "== Hash")
  { type: type, **options } == other
end

# SendConfig
HecksDeprecations.register(HecksPlayground::SendConfig, :[]) do |key|
  HecksDeprecations.warn_deprecated(self.class, "[]")
  case key
  when :name then name
  when :handler then handler
  else options[key]
  end
end

HecksDeprecations.register(HecksPlayground::SendConfig, :==) do |other|
  next super(other) unless other.is_a?(Hash)
  HecksDeprecations.warn_deprecated(self.class, "== Hash")
  { name: name, handler: handler, **options } == other
end

# ExtensionConfig
HecksDeprecations.register(HecksPlayground::ExtensionConfig, :==) do |other|
  next super(other) unless other.is_a?(Hash)
  HecksDeprecations.warn_deprecated(self.class, "== Hash")
  { name: name, **options } == other
end
