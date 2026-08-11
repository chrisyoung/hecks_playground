#!/usr/bin/env ruby
# migrate_bluebook_syntax.rb -- one-off corpus rewrite, run once per target
# tree, output reviewed as a diff before commit (per the standing rule
# against sed/perl bulk renames -- this is a precise script, not a
# text-wide regex).
#
# Transforms, per Part 3 of the migration plan PLUS three more found live
# during governance porting (task 3) that weren't caught by the original
# DSL diff -- each documented at its own method:
#   1. identified_by :field rewrite
#   2. inline lifecycle -> standalone lifecycle
#   3. value-object-typed attribute defaults need hash shape
#   4. query where-clauses need their own declared parameter attribute
#
# Usage: ruby migrate_bluebook_syntax.rb <root> [<root> ...]
#   Walks **/*.bluebook under each root, rewrites in place, prints a summary.
#   Comment lines are never touched.

require "set"

IDENTIFIED_BY_LINE = /\A(\s*)identified_by\s+:(\w+)(?:,\s*suffix:\s*:(\w+))?\s*\z/

def rewrite_identified_by(text)
  changed = false
  lines = text.lines.map do |line|
    next line if line.lstrip.start_with?("#")

    m = IDENTIFIED_BY_LINE.match(line.chomp)
    next line unless m

    changed = true
    indent, field, suffix = m[1], m[2], m[3]
    if suffix
      "#{indent}identified_by { #{field}.value }  # TODO suffix: :#{suffix} -- no direct target-form equivalent yet, see plan Open Questions\n"
    else
      "#{indent}identified_by { #{field}.value }\n"
    end
  end
  [lines.join, changed]
end

# `identified_by TypeConstant` (bare, no colon) -- a SECOND old-syntax form,
# distinct from `identified_by :field` above, found live across the
# other-20-projects wave (migration plan task 8: bin-buddy, opt-website,
# vindiction -- 16 files). Means "identified by whichever field this
# aggregate's own `reference_to TypeConstant` synthesizes" -- reference_to
# always mints `:"#{snake(type)}_id"` (aggregate_builder.rb's own
# cross_reference), so the target field name is fully derivable from the
# type name alone, no cross-referencing the file's own reference_to line
# required.
IDENTIFIED_BY_CONST_LINE = /\A(\s*)identified_by\s+([A-Z]\w*)\s*\z/

def snake_case(name)
  name.gsub(/([a-z\d])([A-Z])/, '\1_\2').gsub(/([A-Z]+)([A-Z][a-z])/, '\1_\2').downcase
end

def rewrite_identified_by_reference(text)
  changed = false
  lines = text.lines.map do |line|
    next line if line.lstrip.start_with?("#")

    m = IDENTIFIED_BY_CONST_LINE.match(line.chomp)
    next line unless m

    changed = true
    indent, type = m[1], m[2]
    field = "#{snake_case(type)}_id"
    "#{indent}identified_by { #{field}.value }\n"
  end
  [lines.join, changed]
end

# Finds `attribute :status, Type, default: "x" do` ... matching `end` and
# either (a) lifts it to a standalone `lifecycle` block right after it, or
# (b) if a standalone `lifecycle :same_field` already exists elsewhere in
# the file, deletes the inline block outright (auth_identity.bluebook's
# shape -- the target already exists, converting would duplicate it).
def rewrite_inline_lifecycle(text)
  return [text, false] unless text =~ /attribute\s+:(status|state),\s*\S+,\s*default:\s*"[^"]*"\s+do\b/

  lines = text.lines
  out = []
  changed = false
  standalone_fields = text.scan(/^\s*lifecycle\s+:(\w+)/).flatten.to_set

  i = 0
  while i < lines.size
    line = lines[i]
    m = /\A(\s*)attribute\s+:(status|state),\s*(\S+),\s*default:\s*"([^"]*)"\s+do\s*\z/.match(line.chomp)
    unless m
      out << line
      i += 1
      next
    end

    indent, field, type, default = m[1], m[2], m[3], m[4]
    body = []
    i += 1
    while i < lines.size && lines[i].strip != "end"
      body << lines[i]
      i += 1
    end
    i += 1 # consume the `end` line

    changed = true
    if standalone_fields.include?(field)
      out << "#{indent}attribute :#{field}, #{type}, default: \"#{default}\"\n"
    else
      out << "#{indent}attribute :#{field}, #{type}, default: \"#{default}\"\n"
      out << "\n"
      out << "#{indent}lifecycle :#{field}, default: \"#{default}\" do\n"
      body.each { |l| out << l }
      out << "#{indent}end\n"
    end
  end
  [out.join, changed]
end

# hecksagain requires a value-object-typed attribute's default: to be a
# hash filling the VO's fields (default: { value: "x" }), not a bare
# scalar (default: "x") -- the old Rust parser auto-wrapped a bare default
# into the VO's single field; hecksagain refuses instead (Malformed: "a
# default fills its FIELDS"). Found live in agent_discipline.bluebook's
# Severity field during governance porting (task 3).
#
# HEURISTIC: assumes the VO's field is named :value (the dominant
# single-field-VO shape in this corpus). A VO with a differently-named or
# multi-field shape needs hand review -- surfaced by the boot-validation
# pass this script's caller runs afterward, not silently mishandled here.
def rewrite_vo_defaults(text)
  vo_names = text.scan(/^\s*value_object\s+"(\w+)"/).flatten.to_set
  return [text, false] if vo_names.empty?

  changed = false
  lines = text.lines.map do |line|
    next line if line.lstrip.start_with?("#")

    # default: matches a quoted string OR a bare literal (0, true, :sym,
    # 1.5, ...) -- both forms need the same { value: ... } hash-wrap.
    # Idempotency guard: a default already shaped `{ ... }` is skipped --
    # without this, re-running the script on an already-fixed file
    # double- and triple-wraps it (found live: `{ value: { value: "x" } }`).
    next line if line.include?("default: {")

    m = /\A(\s*attribute\s+:\w+,\s*)(\w+)(,\s*)default:\s*("[^"]*"|[^,\s][^,]*?)\s*(,.*)?\z/.match(line.chomp)
    next line unless m && vo_names.include?(m[2])

    changed = true
    prefix, type, sep, default_val, suffix = m[1], m[2], m[3], m[4], m[5]
    %(#{prefix}#{type}#{sep}default: { value: #{default_val} }#{suffix}\n)
  end
  [lines.join, changed]
end

# hecksagain requires a query's `where field: :symbol` to be backed by an
# `attribute :symbol, Type` declared INSIDE that same query block -- an
# ask-time parameter, not an implicit reuse of the aggregate's own field
# of that name (the old Rust parser allowed the implicit reuse). Found
# live in agent_discipline.bluebook's ForKind query during governance
# porting (task 3).
#
# HEURISTIC: only handles single-clause `where field: :symbol` (not
# multi-clause or dotted-path wheres), and infers the parameter's Type by
# looking up `field` among the AGGREGATE's own top-level `attribute`
# declarations (the overwhelmingly common case: a query parameter shares
# its name and type with the aggregate field it filters on). A query
# whose parameter name or type doesn't match an aggregate attribute
# 1:1 needs hand review -- surfaced by the boot-validation pass, not
# silently mishandled here.
def rewrite_query_params(text)
  return [text, false] unless text.include?("query \"")

  # Aggregate-level attribute name -> type, keyed off top-level `attribute`
  # lines (2-space-deeper-than-`aggregate` indent in this corpus's style --
  # approximated here as "not inside a value_object/query/command block",
  # tracked via a simple block-depth counter over do/end and nested
  # aggregate/value_object/query/command keywords).
  field_types = {}
  depth = 0
  text.each_line do |line|
    stripped = line.strip
    next if stripped.start_with?("#")

    if stripped =~ /\A(value_object|query|command|entity)\s+"/
      depth += 1
    elsif stripped == "end" && depth.positive?
      depth -= 1
    elsif depth.zero? && (m = /\Aattribute\s+:(\w+),\s*(\w+)/.match(stripped))
      field_types[m[1]] = m[2]
    end
  end
  return [text, false] if field_types.empty?

  lines = text.lines
  out = []
  changed = false
  in_query = false
  query_declared_params = Set.new
  query_start_index = nil

  lines.each_with_index do |line, idx|
    stripped = line.strip

    if stripped =~ /\Aquery\s+"/
      in_query = true
      query_declared_params = Set.new
      query_start_index = out.size
      out << line
      next
    end

    if in_query
      if (m = /\Aattribute\s+:(\w+)/.match(stripped))
        query_declared_params << m[1]
      elsif (m = /\Awhere\s+(\w+):\s*:(\w+)\s*\z/.match(stripped))
        field, param = m[1], m[2]
        if !query_declared_params.include?(param) && field_types.key?(field)
          indent = line[/\A\s*/]
          out.insert(query_start_index + 1, "#{indent}attribute :#{param}, #{field_types[field]}\n")
          query_start_index += 1
          changed = true
        end
      elsif stripped == "end"
        in_query = false
      end
    end

    out << line
  end

  [out.join, changed]
end

# hecksagain's EntityBuilder requires every entity to declare its OWN
# identified_by ("an entity says what it is known by") -- the old Rust
# parser allowed an entity with no identified_by at all (implicit /
# unenforced identity). Found live in macrophage.bluebook, where every
# one of ~35 entities has `attribute :id, XxxId` as a plain field but no
# identified_by declaration.
#
# HEURISTIC: for each `entity "Name" do ... end` block with no
# identified_by of its own, if the block declares `attribute :id, Type`,
# insert `identified_by { id.value }` right after the entity's own
# opening line (before description, matching this corpus's own
# aggregate-level convention of identified_by first). An entity with no
# :id attribute at all needs hand review -- surfaced by the
# boot-validation pass, not silently mishandled here.
def rewrite_entity_identity(text)
  return [text, false] unless text.include?("entity \"")

  lines = text.lines
  out = []
  changed = false
  i = 0
  while i < lines.size
    line = lines[i]
    unless line.strip =~ /\Aentity\s+"\w+"/
      out << line
      i += 1
      next
    end

    indent = line[/\A\s*/]
    body_indent = indent + "  "
    # Collect this entity's own body (depth-tracked over nested do/end --
    # value_object/command/query bodies inside an entity all close with
    # `end` too, so track depth generically).
    body = []
    depth = 0
    i += 1
    while i < lines.size
      l = lines[i]
      stripped = l.strip
      if stripped.end_with?(" do") || stripped == "do"
        depth += 1
      elsif stripped == "end"
        break if depth.zero?

        depth -= 1
      end
      body << l
      i += 1
    end
    entity_end_line = lines[i]
    i += 1

    out << line
    if body.none? { |l| l.strip =~ /\Aidentified_by\b/ }
      id_field = body.find { |l| l.strip =~ /\Aattribute\s+:id,/ } && "id"
      # Fallback: no plain :id field -- use the FIRST declared attribute as
      # identity (common convention for entities keyed by their one
      # meaningful field, e.g. Violation's :check_id). A genuinely
      # composite identity (more than one field needed) still needs hand
      # review -- this heuristic picks one field, which is either exactly
      # right or a clearly-visible wrong guess the boot-validation pass
      # surfaces immediately (a Malformed on a nonexistent field, or a
      # collision on re-dispatch), not a silent wrong answer.
      id_field ||= body.find { |l| l.strip =~ /\Aattribute\s+:(\w+),/ } && $1
      if id_field
        out << "#{body_indent}identified_by { #{id_field}.value }\n"
        changed = true
      end
    end
    body.each { |l| out << l }
    out << entity_end_line
  end
  [out.join, changed]
end

def migrate_file(path)
  original = File.read(path)
  text = original
  tags = {}

  text, tags[:identified_by]     = rewrite_identified_by(text)
  text, tags[:identified_by_ref] = rewrite_identified_by_reference(text)
  text, tags[:lifecycle]         = rewrite_inline_lifecycle(text)
  text, tags[:vo_defaults]       = rewrite_vo_defaults(text)
  text, tags[:query_params]      = rewrite_query_params(text)
  text, tags[:entity_identity]   = rewrite_entity_identity(text)

  return nil unless tags.values.any?

  File.write(path, text)
  tags.merge(path: path)
end

if __FILE__ == $0
  roots = ARGV.empty? ? ["."] : ARGV
  results = roots.flat_map { |root| Dir.glob(File.join(root, "**", "*.bluebook")) }
                 .sort
                 .filter_map { |f| migrate_file(f) }

  puts "#{results.size} files changed:"
  results.each do |r|
    tags = [:identified_by, :identified_by_ref, :lifecycle, :vo_defaults, :query_params, :entity_identity].select { |k| r[k] }.join(", ")
    puts "  #{r[:path]}  (#{tags})"
  end
end
