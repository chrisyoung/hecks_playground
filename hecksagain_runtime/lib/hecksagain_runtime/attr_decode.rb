# attr_decode.rb — turn a dispatch/query arg STRING into a Ruby value, with
# the attribute's DECLARED SHAPE in hand, instead of guessing.
#
# hecksagain-cli's `parse_kv` used to `JSON.parse` every `k=v` value blind,
# falling back to the raw string only on a parse error. That guess is right
# for a genuinely structural attribute -- a nested value object
# (`daily_limit={"cents":5000,"currency":{"code":"USD"}}`) must arrive as a
# Hash or no given can read it -- and wrong for a plain TEXT holder whose
# value simply happens to look like JSON:
#
#   hecksagain-cli dispatch root Tools::FileTool.Write file_path=x.json content='{"a":1}'
#
# `content` is declared a one-field text value object, but its VALUE looked
# like JSON, so the blind parse decoded it into a Hash. Downstream, the
# Filesystem adapter rendered that Hash and wrote THAT to disk -- silently
# replacing any JSON file written through the door with a `#to_s` summary of
# itself, while still reporting success. Same class of bug the retired Rust
# `attr_decode.rs` (commit fc4525f92, deleted in the hecksagain-cutover) was
# written to close for the old runtime; this is that fix, ported.
#
# THE DECLARED TYPE SETTLES IT:
#
#   String, or a value object with ONE String field   TEXT — kept verbatim
#   anything else (Money { cents, currency })          SHAPE — decoded
#
# A one-field text holder has no nesting to decode INTO, so whatever the
# caller wrote IS the value.
module HecksagainRuntime
  module AttrDecode
    module_function

    # Resolve `verb`'s attribute list against the booted `runtime`'s
    # registry (a command's `.attributes`, or an aggregate-scoped query's).
    # Returns [aggregate_ir, attributes] ; either half is nil when `verb`
    # doesn't resolve (unknown domain/aggregate/verb, or a domain-level
    # read model with no owning aggregate) -- callers treat that as "decode
    # nothing specially," never as an error, so an unresolvable verb keeps
    # deciding downstream exactly as it always did.
    def resolve_attributes(runtime, verb, kind:)
      domain_name, aggregate_name, member_name = Hecks::Naming.split_verb(verb)
      return [nil, nil] unless domain_name && aggregate_name && member_name

      bluebook  = runtime.registry.bluebook(domain_name)
      aggregate = bluebook&.aggregate(aggregate_name)
      return [nil, nil] unless aggregate

      members = kind == :query ? aggregate.queries : aggregate.commands
      member  = members&.find { |m| m.hecks_name == member_name }
      [aggregate, member&.attributes]
    rescue StandardError
      [nil, nil]
    end

    # The names, among `attributes`, whose declared type carries TEXT
    # rather than structure -- see the header above.
    def text_typed_names(aggregate, attributes)
      return [].to_set unless aggregate && attributes

      attributes.reject(&:list?)
                .select { |a| text_type?(aggregate, a.type) }
                .map { |a| a.name.to_s }
                .to_set
    end

    def text_type?(aggregate, attr_type)
      return true if attr_type.to_s == "String"
      return false unless aggregate.respond_to?(:value_object)

      vo = aggregate.value_object(attr_type)
      return false unless vo

      vo.attributes.size == 1 && !vo.attributes.first.list? && vo.attributes.first.type.to_s == "String"
    end

    # Decode a whole raw k=v arg hash (string values) for `verb`, honouring
    # each attribute's declared shape: TEXT attributes stay verbatim
    # strings ; everything else keeps the prior best-effort
    # JSON.parse-with-fallback guess. Safe to call with already-typed
    # (non-String) values too -- `loose_json_parse` passes those through
    # unchanged, so a caller that isn't the raw-string CLI path is unaffected.
    def decode_args(runtime, verb, raw_args, kind:)
      aggregate, attributes = resolve_attributes(runtime, verb, kind: kind)
      text_names = text_typed_names(aggregate, attributes)

      raw_args.each_with_object({}) do |(k, v), decoded|
        decoded[k] = text_names.include?(k.to_s) ? v : loose_json_parse(v)
      end
    end

    def loose_json_parse(value)
      return value unless value.is_a?(String)

      JSON.parse(value)
    rescue JSON::ParserError, TypeError
      value
    end
  end
end
