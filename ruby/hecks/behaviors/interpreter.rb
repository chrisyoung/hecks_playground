# Hecks::Behaviors::Interpreter
#
# Evaluates given expressions and applies mutations against an
# AggregateState. Mirrors rust/src/runtime/interpreter.rs
# exactly so the Ruby and Rust runners agree on every test.
#
# Operators supported (highest → lowest precedence):
#   field.any?, field.empty?    Ruby idioms
#   ==, !=, >=, <=, >, <        comparisons
#   &&, ||                      boolean (top-level split, && binds tighter)
#
# Mutation values resolve symbols (`:attr`) from attrs then state,
# numbers as Int, quoted strings, true/false, [] / [a, b, c] as List,
# {k: v} as Map. Anything else falls through to Str.
#
#   Interpreter.check_givens(cmd, state, attrs)
#   Interpreter.apply_mutations(cmd, state, attrs)
require_relative "value"

module Hecks
  module Behaviors
    module Interpreter
      module_function

      # The aggregate whose command is being judged — held for the duration of
      # a dispatch so a given can reach its value objects' derivations.
      def with_aggregate(agg)
        previous = @current_aggregate
        @current_aggregate = agg
        yield
      ensure
        @current_aggregate = previous
      end

      def check_givens(cmd, state, attrs)
        cmd.givens.each do |g|
          next if evaluate_given(g.expression, state, attrs)
          msg = (g.respond_to?(:message) && g.message) || g.expression
          raise GivenFailed.new(msg, g.expression)
        end
      end

      # Aggregate-level invariants (f4) : a `given` checked on EVERY command,
      # evaluated against the RESULTING state. The first violated invariant
      # rejects the command exactly like a failed given — raised as GivenFailed
      # with the invariant NAME as message so `expect refused:` matches the
      # name (mirror rust/src/invariants + behaviors_runner InvariantViolation).
      # Enforce `required: true` command attributes (before givens). A required
      # kwarg that is absent, Null, or empty refuses the command — the structural
      # superset of the old `given { x != "" }` idiom which couldn't see a truly
      # absent (nil) kwarg. Mirror rust interpreter::check_required.
      def check_required(cmd, attrs)
        (cmd.attributes || []).each do |a|
          next unless a.respond_to?(:required) && a.required
          v = attrs[a.name.to_s]
          present = v && !(v.respond_to?(:null?) && v.null?) && v.to_display.to_s != ""
          next if present
          raise GivenFailed.new("#{a.name} is required", "required")
        end
      end

      def check_invariants(agg, state, attrs)
        invariants = agg.respond_to?(:invariants) ? (agg.invariants || []) : []
        invariants.each do |inv|
          next unless inv.respond_to?(:expression) && !inv.expression.nil?
          next if evaluate_given(inv.expression, state, attrs)
          raise GivenFailed.new(inv.message.to_s, inv.expression)
        end
      end

      def apply_mutations(cmd, state, attrs)
        cmd.mutations.each do |m|
          op = m.operation.to_sym
          val = resolve_native_value(m.value, attrs, state)
          case op
          when :set       then state.set(m.field, val)
          when :append    then state.append(m.field, val)
          when :increment then increment_field(state, m.field, val)
          when :decrement then increment_field(state, m.field, val, sign: -1)
          when :toggle    then state.toggle(m.field)
          end
        end
      end

      # Float-aware increment/decrement so `then_set :fatigue,
      # increment: 0.01` actually adds 0.01 (rather than rounding to 1
      # via `.to_i`). Mirrors rust/src/runtime/aggregate_state.rs
      # increment_float.
      def increment_field(state, field, val, sign: 1)
        # Value-object arithmetic first : Money += Money adds cents, carries
        # currency. Mirrors rust/src/runtime/interpreter.rs vo_arith — without
        # it the Money map collapsed to Int(1).
        if (vo = vo_arith(state.get(field), val, sign))
          state.set(field, vo)
          return
        end
        amount = val.numeric || 1
        if amount.to_i.to_f == amount
          state.increment(field, amount.to_i * sign)
        else
          state.increment_float(field, amount * sign)
        end
      end

      # Value-object arithmetic : when the field holds a value object (:map)
      # AND the delta is a value object, combine numeric fields pairwise
      # (cents += cents), carrying non-numeric fields (currency) from the
      # field. Returns nil unless both sides are :map. Mirrors Rust vo_arith.
      def vo_arith(current, delta, sign)
        current = Value.from(current) unless current.is_a?(Value)
        return nil unless current.kind == :map && delta.is_a?(Value) && delta.kind == :map
        out = current.raw.dup
        delta.raw.each do |k, dv|
          cn = out[k].is_a?(Value) ? out[k].numeric : nil
          dn = dv.is_a?(Value) ? dv.numeric : nil
          out[k] = Value.new(:int, (cn + sign * dn).to_i) if cn && dn
        end
        Value.new(:map, out)
      end

      # Ruby IR preserves mutation value types: Symbol → attr ref,
      # String → literal, Integer/Float/Bool/Array/Hash → literal.
      # Falls through to string-form resolution for anything stringy
      # (Rust IR carries source tokens like "\"literal\"" / ":attr").
      def resolve_native_value(value, attrs, state)
        case value
        when Symbol
          v = attrs[value.to_s] || attrs[value] || state.fields[value.to_s]
          return v.is_a?(Value) ? v : Value.from(v)
        when String
          # Could be a Rust-style source token (`":foo"`, `"\"x\""`)
          # OR a Ruby string literal. Source tokens start with `:` /
          # `"` / `[` / `{` — fall through to string-form resolver.
          stripped = value.strip
          if stripped.start_with?(':', '"', '[', '{')
            return resolve_mutation_value(value, attrs, state)
          end
          # Bare numeric/bool sneaking in as String — let str-form handle.
          return resolve_mutation_value(value, attrs, state) if %w[true false].include?(stripped) ||
                                                                Integer(stripped, exception: false) ||
                                                                Float(stripped, exception: false)
          # Otherwise: literal string.
          Value.from(value)
        when Integer, Float, TrueClass, FalseClass, NilClass
          Value.from(value)
        when Array
          Value.list(value)
        when Hash
          Value.from(value)
        else
          Value.from(value.to_s)
        end
      end

      def evaluate_given(expr, state, attrs)
        expr = expr.to_s.strip
        # Boolean operators — split lowest precedence first.
        if (parts = split_top_level(expr, "||"))
          return evaluate_given(parts[0], state, attrs) || evaluate_given(parts[1], state, attrs)
        end
        if (parts = split_top_level(expr, "&&"))
          return evaluate_given(parts[0], state, attrs) && evaluate_given(parts[1], state, attrs)
        end
        # Ruby idioms: any?/empty? rewrite to size comparisons.
        if expr.end_with?(".any?")
          field = expr[0..-6].strip
          return Value.lt?(Value.from(0), resolve_expr("#{field}.size", state, attrs))
        end
        if expr.end_with?(".empty?")
          field = expr[0..-8].strip
          return Value.equal?(resolve_expr("#{field}.size", state, attrs), Value.from(0))
        end
        # Comparisons — multi-char before single-char.
        %w[>= <= < > == !=].each do |op|
          parts = split_comparison(expr, op)
          next unless parts
          left  = resolve_expr(parts[0].strip, state, attrs)
          right = resolve_expr(parts[1].strip, state, attrs)
          case op
          when ">=" then return !Value.lt?(left, right)
          when "<=" then return !Value.lt?(right, left)
          when "<"  then return Value.lt?(left, right)
          when ">"  then return Value.lt?(right, left)
          when "==" then return Value.equal?(left, right)
          when "!=" then return !Value.equal?(left, right)
          end
        end
        # A RICH VALUE OBJECT's own predicate — `amount.positive?`,
        # `balance.covers?(amount)`, `amount.same_currency?(balance)`. Not
        # implemented here at all, these fell to the `true` below and every
        # money guard in banking silently admitted whatever it was given :
        # a zero deposit, an overdraft, a withdrawal over the daily limit.
        # Rust evaluates them (interp_expr::lookup_derivation), so the same
        # bluebook refused there and passed here — the single largest source
        # of banking's runtime disagreement.
        verdict = derivation_verdict(expr, state, attrs)
        return verdict unless verdict.nil?

        true
      end

      # Evaluate `receiver.method(args...)` when `method` names a `derive`
      # on the receiver's value object. Ruby keeps the derivation as a PROC,
      # so it is CALLED with the receiver's fields in scope rather than
      # re-parsed — the body says `cents >= other.cents` and means it.
      # Returns nil when this is not a derivation, so the caller falls
      # through unchanged.
      def derivation_verdict(expr, state, attrs)
        agg = @current_aggregate
        return nil unless agg && expr.end_with?(")") || (agg && expr.end_with?("?"))

        head, arg_src = if expr.end_with?(")")
                          i = expr.index("(")
                          return nil unless i
                          [expr[0...i], expr[(i + 1)..-2]]
                        else
                          [expr, nil]
                        end
        dot = head.rindex(".")
        return nil unless dot

        receiver_name = head[0...dot].strip
        method_name   = head[(dot + 1)..].strip
        return nil if receiver_name.empty? || receiver_name.include?(".")

        receiver = attrs[receiver_name] || attrs[receiver_name.to_sym] || state.get(receiver_name)
        return nil unless receiver.respond_to?(:kind) && receiver.kind == :map

        vo = agg.value_objects.find { |v|
          v.respond_to?(:derivations) &&
            v.derivations.any? { |d| d.name.to_s == method_name } &&
            v.attributes.all? { |a| receiver.raw.key?(a.name.to_s) }
        }
        deriv = vo&.derivations&.find { |d| d.name.to_s == method_name }
        return nil unless deriv&.respond_to?(:block) && deriv.block

        args = (arg_src.to_s.empty? ? [] : split_top_level_commas(arg_src)).map { |a|
          wrap_for_derivation(resolve_expr(a.strip, state, attrs))
        }
        !!VOScope.new(receiver.raw).instance_exec(*args, &deriv.block)
      rescue StandardError
        # A derivation that cannot be evaluated is not a silent pass — let
        # the caller's own default decide, and keep the failure visible.
        nil
      end

      def wrap_for_derivation(value)
        return VOScope.new(value.raw) if value.respond_to?(:kind) && value.kind == :map

        value.respond_to?(:raw) ? value.raw : value
      end

      def split_top_level_commas(src)
        parts = []
        depth = 0
        buf = +""
        src.each_char do |c|
          case c
          when "(", "{", "[" then depth += 1
          when ")", "}", "]" then depth -= 1
          end
          if c == "," && depth.zero?
            parts << buf
            buf = +""
          else
            buf << c
          end
        end
        parts << buf unless buf.strip.empty?
        parts
      end

      # The `self` a derivation body runs against : the value object's own
      # fields, answered as plain Ruby so `cents >= other.cents` compares
      # numbers, and nested value objects answered as another scope so
      # `currency.code` keeps walking.
      class VOScope
        def initialize(fields) = @fields = fields

        def method_missing(name, *_args)
          key = name.to_s
          return super unless @fields.key?(key)

          v = @fields[key]
          v.respond_to?(:kind) && v.kind == :map ? VOScope.new(v.raw) : (v.respond_to?(:raw) ? v.raw : v)
        end

        def respond_to_missing?(name, _priv = false) = @fields.key?(name.to_s)
      end

      def split_top_level(expr, op)
        in_str = false
        i = 0
        while i + op.length <= expr.length
          c = expr[i]
          in_str = !in_str if c == '"'
          if !in_str && expr[i, op.length] == op
            return [expr[0...i].strip, expr[(i + op.length)..].strip]
          end
          i += 1
        end
        nil
      end

      def split_comparison(expr, op)
        return nil if op == "<" && (expr.include?("<=") || expr.include?("<<"))
        return nil if op == ">" && (expr.include?(">=") || expr.include?(">>"))
        idx = expr.index(op)
        return nil unless idx
        [expr[0...idx], expr[(idx + op.length)..]]
      end

      def resolve_expr(expr, state, attrs)
        return Value.from(Integer(expr)) if Integer(expr, exception: false)
        if expr.start_with?('"') && expr.end_with?('"')
          return Value.from(expr[1..-2])
        end
        return Value.from(true)  if expr == "true"
        return Value.from(false) if expr == "false"
        # rand_below(N) — uniform random integer in [0, N). Mirrors the
        # Rust evaluator (rust/src/runtime/interpreter.rs) so
        # `given { rand_below(N) == 0 }` evaluates the same in both
        # runners. HECKS_RAND_SEED env var overrides RNG for tests :
        # seed=0 makes the predicate fire (always returns 0) ; seed=k
        # always returns k % N.
        if expr.start_with?("rand_below(") && expr.end_with?(")")
          arg = expr[("rand_below(".length)..-2].strip
          arg_val = resolve_expr(arg, state, attrs)
          n = arg_val.numeric.to_i rescue 0
          return Value.from(0) if n <= 0
          if (seed = ENV["HECKS_RAND_SEED"]) && Integer(seed, exception: false)
            return Value.from(Integer(seed) % n)
          end
          return Value.from(rand(n))
        end
        if expr.end_with?(".size")
          field = expr[0...-5]
          val = attrs[field] || attrs[field.to_sym] || state.get(field)
          v = Value.from(val)
          return Value.from(v.list_size) if v.list?
          return Value.from(v.str_size)  if v.kind == :str
          return Value.from(0)
        end
        # <expr>.modulo(N) — periodic cadence gate. Resolves the receiver
        # as an integer (attr / state field / literal) and returns
        # `receiver % N`. Mirrors rust/src/runtime/interpreter.rs so
        # `given { tick.modulo(60) == 0 }` evaluates the same in both
        # runners. Non-positive N short-circuits to 0 (predicate fires
        # every call) — same safer-than-panicking guard as rand_below.
        if (modulo_idx = expr.rindex(".modulo("))
          if expr.end_with?(")")
            receiver = expr[0...modulo_idx]
            arg = expr[(modulo_idx + ".modulo(".length)...-1]
            recv_val = resolve_expr(receiver.strip, state, attrs)
            arg_val  = resolve_expr(arg.strip, state, attrs)
            n = (arg_val.numeric.to_i rescue 0)
            return Value.from(0) if n <= 0
            lhs = (recv_val.numeric.to_i rescue 0)
            return Value.from(lhs % n)
          end
        end
        # Dotted value-object field access : `amount.cents`,
        # `amount.currency.code`. Resolve the head from attrs (input shadows
        # state) or state, then step into each :map segment. Mirrors
        # rust/src/runtime/interpreter.rs. Only ADDS nested navigation ; a
        # non-map head falls through to the flat lookup below.
        if expr.include?(".")
          head, path = expr.split(".", 2)
          base = attrs[head] || attrs[head.to_sym] || state.fields[head] || state.fields[head.to_sym]
          unless base.nil?
            cur = Value.from(base)
            navigated = true
            path.split(".").each do |seg|
              if cur.kind == :map && (cur.raw.key?(seg) || cur.raw.key?(seg.to_sym))
                cur = cur.raw[seg] || cur.raw[seg.to_sym]
              else
                navigated = false
                break
              end
            end
            return cur if navigated
          end
        end
        if (v = attrs[expr] || attrs[expr.to_sym])
          return Value.from(v)
        end
        Value.from(state.get(expr))
      end

      def resolve_mutation_value(expr, attrs, state)
        expr = expr.to_s.strip
        if expr.start_with?('{') && expr.end_with?('}')
          inner = expr[1..-2]
          map = {}
          inner.split(',').each do |pair|
            pair = pair.strip
            if (idx = pair.index(':'))
              key = pair[0...idx].strip.delete_prefix(':')
              ref = pair[(idx + 1)..].strip.delete_prefix(':')
              map[key] = attrs[ref] || attrs[ref.to_sym] || ref
            end
          end
          return Value.from(map)
        end
        if expr.start_with?(':')
          field = expr[1..]
          val = attrs[field] || attrs[field.to_sym] || state.fields[field]
          return val.is_a?(Value) ? val : Value.from(val)
        end
        return Value.from(Integer(expr)) if Integer(expr, exception: false)
        if expr.start_with?('"') && expr.end_with?('"')
          return Value.from(expr[1..-2])
        end
        if expr.start_with?('[') && expr.end_with?(']')
          inner = expr[1..-2].strip
          return Value.list([]) if inner.empty?
          items = inner.split(',').map { |item| resolve_mutation_value(item.strip, attrs, state) }
          return Value.new(:list, items)
        end
        return Value.from(true)  if expr == "true"
        return Value.from(false) if expr == "false"
        # Bare identifier — try attrs, fall through to literal string.
        val = attrs[expr] || attrs[expr.to_sym]
        return val.is_a?(Value) ? val : Value.from(val) if val
        Value.from(expr)
      end

      class GivenFailed < StandardError
        attr_reader :expression
        def initialize(message, expression)
          super(message)
          @expression = expression
        end
      end
    end
  end
end
