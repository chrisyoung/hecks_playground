//! HecksalInterpreter — evaluates givens and applies mutations
//!
//! The expression evaluator for Bluebook's declarative behavior.
//! Givens are predicates. Mutations are state changes. Both are data.
//!
//! Usage:
//!   check_givens(cmd, state, attrs, ctx)?;   // ctx carries VO type info
//!   apply_mutations(cmd, state, attrs);
//!
//! [antibody-exempt: i106 dsl-mutation-primitives — kernel-surface
//!  runtime extension that applies Multiply / Clamp / Decay. Same
//!  retirement contract as ir.rs.]
//!
//! [antibody-exempt: rand_below(N) predicate primitive — stochastic
//!  dispatch gates (RANDOM % N == 0, every-Nth-tick) move from
//!  shell-side into bluebook givens. Lets surface_musing /
//!  musing_mint / daydream fire end-to-end via bluebook. Same i80
//!  retirement contract.]
//!
//! [antibody-exempt: i229 tick.modulo(N) periodic cadence primitive —
//!  `<expr>.modulo(N)` resolves the receiver as an integer and returns
//!  `receiver % N`, lifting `if (tick % N).zero?` shell-side gates into
//!  declarative bluebook givens. Mirrors the rand_below shape : same
//!  retirement contract, same Ruby parity mirror in
//!  ruby/hecks/behaviors/interpreter.rb.]

use super::interp_expr::resolve_expr;
use super::interp_expr_ops::{
    compare_lt, numeric_value, split_comparison, split_top_level, values_equal,
};
use super::{AggregateState, RuntimeError, Value};
use crate::ir::{Aggregate, Command, Derivation, MutationOp, ValueObject};
use std::collections::HashMap;

/// The borrow-only TYPE context a predicate evaluates against. A runtime
/// `Value::Map` carries no type tag, so a bare `balance` cannot know it is a
/// `Money` that declares `covers?`. EvalCtx supplies that : `attr_types` maps an
/// attribute NAME to its value-object type, and `value_objects` is the aggregate's
/// VO list to resolve the derivation on. It is Copy (two borrows) and threads
/// unchanged through every recursive `resolve_expr` / `evaluate_given` call.
/// `EvalCtx::default()` is EMPTY — no types, no VOs — so the invariants and
/// payload-gate callers (which judge incoming attrs with no VO-method dispatch)
/// pass it and the method-dispatch branch stays inert there.
#[derive(Clone, Copy, Default)]
pub struct EvalCtx<'a> {
    value_objects: &'a [ValueObject],
    attr_types: Option<&'a HashMap<String, String>>,
}

impl<'a> EvalCtx<'a> {
    /// The type context for a command's givens : the aggregate's value objects
    /// plus a name→type map spanning the command's attributes and the aggregate's
    /// own attributes (state fields). Command attrs shadow aggregate attrs on a
    /// name collision, mirroring `resolve_expr`'s attrs-shadow-state rule.
    pub fn for_command(vos: &'a [ValueObject], attr_types: &'a HashMap<String, String>) -> Self {
        EvalCtx { value_objects: vos, attr_types: Some(attr_types) }
    }
}

/// Build the name→value-object-type map a command's givens resolve against.
/// Aggregate attributes first (state fields), then command attributes so a
/// same-named command kwarg shadows the state field — the attrs-shadow-state
/// rule `resolve_expr` already honours.
pub fn build_attr_types(cmd: &Command, agg: &Aggregate) -> HashMap<String, String> {
    let mut types = HashMap::new();
    for a in &agg.attributes {
        types.insert(a.name.clone(), a.attr_type.clone());
    }
    for a in &cmd.attributes {
        types.insert(a.name.clone(), a.attr_type.clone());
    }
    types
}

/// Resolve a `receiver.method` call to the derivation it names — iff a type
/// context is present, the receiver attribute has a known value-object type,
/// and that VO declares a derivation of that name. Returns None (so the caller
/// falls through to field access / flat lookup) in every other case.
pub(super) fn lookup_derivation<'a>(ctx: EvalCtx<'a>, receiver: &str, method: &str) -> Option<&'a Derivation> {
    let types = ctx.attr_types?;
    let vo_name = types.get(receiver)?;
    let vo = ctx.value_objects.iter().find(|v| &v.name == vo_name)?;
    vo.derivations.iter().find(|d| d.name == method)
}

pub fn check_givens(
    cmd: &Command,
    state: &AggregateState,
    attrs: &HashMap<String, Value>,
    ctx: EvalCtx,
) -> Result<(), RuntimeError> {
        // A mutation the parser could not resolve (an unknown `then_set` op like
        // `bogus_op:`) is recorded with `invalid_op = Some(_)` instead of
        // panicking the parse. Refuse to dispatch it : applying the placeholder
        // `Set` would silently write the wrong value — the exact silent failure
        // the graceful-record path exists to avoid. `validate` catches this
        // earlier ; this is the runtime's own guard for callers that skip it.
        for m in &cmd.mutations {
            if let Some(op) = &m.invalid_op {
                return Err(RuntimeError::GivenFailed {
                    message: format!(
                        "unknown mutation op `{}:` in then_set for `:{}` — valid ops are \
                         to, append, append_unique, remove, increment, decrement, \
                         multiply, clamp, decay, from",
                        op, m.field
                    ),
                    expression: "invalid_mutation_op".to_string(),
                });
            }
        }
        // `required: true` attributes are enforced before givens — a kwarg that is
    // absent, Null, or empty refuses the command the same shape a failed given
    // does. Structural superset of the old `given { x != "" }` idiom, which
    // could not see a truly-absent (Null) kwarg.
    for attr in &cmd.attributes {
        if attr.required {
            let present = attrs.get(&attr.name)
                .is_some_and(|v| !matches!(v, Value::Null) && v.to_string() != "");
            if !present {
                return Err(RuntimeError::GivenFailed {
                    message: format!("{} is required", attr.name),
                    expression: "required".to_string(),
                });
            }
        }
    }
    for given in &cmd.givens {
        if !evaluate_given(&given.expression, state, attrs, ctx) {
            return Err(RuntimeError::GivenFailed {
                message: given
                    .message
                    .clone()
                    .unwrap_or_else(|| given.expression.clone()),
                expression: given.expression.clone(),
            });
        }
    }
    Ok(())
}

pub fn apply_mutations(
    cmd: &Command,
    state: &mut AggregateState,
    attrs: &HashMap<String, Value>,
) {
    for mutation in &cmd.mutations {
        match mutation.operation {
            MutationOp::Set => {
                let val = resolve_mutation_value(&mutation.value, attrs, state);
                state.set(&mutation.field, val);
            }
            MutationOp::Append => {
                let val = resolve_mutation_value(&mutation.value, attrs, state);
                state.append(&mutation.field, val);
            }
            MutationOp::Increment => {
                let val = resolve_mutation_value(&mutation.value, attrs, state);
                // Value-object arithmetic first : Money += Money adds cents,
                // carries currency — without this the Map collapsed to Int(1).
                if let Some(vo) = vo_arith(state.get(&mutation.field), &val, 1) {
                    state.set(&mutation.field, vo);
                    continue;
                }
                // Float-valued increment (e.g. fatigue += 0.01) needs
                // float arithmetic — `as_int()` would silently round it.
                if let Some(f) = numeric_value(&val) {
                    if f.fract() != 0.0 {
                        state.increment_float(&mutation.field, f);
                        continue;
                    }
                }
                let amount = val.as_int().unwrap_or(1);
                state.increment(&mutation.field, amount);
            }
            MutationOp::Decrement => {
                let val = resolve_mutation_value(&mutation.value, attrs, state);
                if let Some(vo) = vo_arith(state.get(&mutation.field), &val, -1) {
                    state.set(&mutation.field, vo);
                    continue;
                }
                if let Some(f) = numeric_value(&val) {
                    if f.fract() != 0.0 {
                        state.decrement_float(&mutation.field, f);
                        continue;
                    }
                }
                let amount = val.as_int().unwrap_or(1);
                state.decrement(&mutation.field, amount);
            }
            MutationOp::Toggle => {
                state.toggle(&mutation.field);
            }
            MutationOp::Delete => {
                // Record-level — flag the state so the dispatcher
                // calls Repository::delete instead of save.
                state.deleted = true;
            }
            MutationOp::Multiply => {
                // i106 — current * factor. Factor source-text is
                // resolved against attrs/state first so dispatchers can
                // pass it as a command attribute (`then_set :s, multiply: :rate`)
                // when the factor isn't a literal.
                let val = resolve_mutation_value(&mutation.value, attrs, state);
                if let Some(factor) = numeric_value(&val) {
                    let cur = field_numeric(&mutation.field, state);
                    state.set_float(&mutation.field, cur * factor);
                }
            }
            MutationOp::Decay => {
                // i106 — current * (1 - rate). Rate is the source-text
                // f64 (e.g. 0.05 → ×0.95). Decay composes with Clamp on
                // the same field — body math typically decays then clamps.
                let val = resolve_mutation_value(&mutation.value, attrs, state);
                if let Some(rate) = numeric_value(&val) {
                    let cur = field_numeric(&mutation.field, state);
                    state.set_float(&mutation.field, cur * (1.0 - rate));
                }
            }
            MutationOp::Clamp => {
                // i106 — bound the field to [min, max]. Value carries a
                // 2-element list literal; resolve_mutation_value returns
                // a Value::List which we read pairwise. Out-of-range
                // clamps to the boundary; in-range is a no-op.
                let bounds = resolve_mutation_value(&mutation.value, attrs, state);
                if let Some((min, max)) = clamp_bounds(&bounds) {
                    let cur = field_numeric(&mutation.field, state);
                    let bounded = cur.max(min).min(max);
                    state.set_float(&mutation.field, bounded);
                }
            }
            MutationOp::Remove => {
                let val = resolve_mutation_value(&mutation.value, attrs, state);
                state.remove(&mutation.field, val);
            }
            MutationOp::AppendUnique => {
                let val = resolve_mutation_value(&mutation.value, attrs, state);
                state.append_unique(&mutation.field, val);
            }
        }
    }
}

/// Numeric read of a state field, used by Multiply/Decay/Clamp. Falls
/// Value-object arithmetic : when the field currently holds a value object
/// (Value::Map) AND the delta is a value object of the same shape, combine their
/// NUMERIC fields pairwise (Money.cents += amount.cents), carrying the field's
/// non-numeric fields (currency) unchanged. `sign` is +1 for increment, -1 for
/// decrement. Returns None when either side isn't a Map — the caller falls back
/// to scalar arithmetic. This is what lets `then_set :balance, increment: :amount`
/// add two Money value objects instead of collapsing balance to a bare integer.
fn vo_arith(current: &Value, delta: &Value, sign: i64) -> Option<Value> {
    let (Value::Map(cur), Value::Map(d)) = (current, delta) else {
        return None;
    };
    let mut out = cur.clone();
    for (k, dv) in d {
        if let (Some(cn), Some(dn)) = (out.get(k).and_then(numeric_value), numeric_value(dv)) {
            // both fields numeric — combine (Money is integer cents).
            out.insert(k.clone(), Value::Int((cn + sign as f64 * dn) as i64));
        }
        // a non-numeric field (currency) keeps the field's own value in `out`.
    }
    Some(Value::Map(out))
}

/// back to 0.0 when the field is unset or non-numeric — matches the
/// existing increment_float behaviour.
fn field_numeric(field: &str, state: &AggregateState) -> f64 {
    numeric_value(state.get(field)).unwrap_or(0.0)
}

/// Read [min, max] from a Value::List of two numeric elements. Returns
/// None when the shape doesn't fit — caller treats that as a no-op.
fn clamp_bounds(v: &Value) -> Option<(f64, f64)> {
    if let Value::List(items) = v {
        if items.len() == 2 {
            let lo = numeric_value(&items[0])?;
            let hi = numeric_value(&items[1])?;
            return Some((lo.min(hi), lo.max(hi)));
        }
    }
    None
}

/// f4 — evaluate a single predicate against state + attrs, returning its
/// truth value. Public wrapper over the `given` evaluator so the invariants
/// module can reuse the exact same predicate machinery a `given` uses ; an
/// invariant IS a `given` checked on every command's resulting state.
pub fn evaluate_predicate(
    expr: &str,
    state: &AggregateState,
    attrs: &HashMap<String, Value>,
    ctx: EvalCtx,
) -> bool {
    evaluate_given(expr, state, attrs, ctx)
}

pub(super) fn evaluate_given(
    expr: &str,
    state: &AggregateState,
    attrs: &HashMap<String, Value>,
    ctx: EvalCtx,
) -> bool {
    let expr = expr.trim();

    // Boolean operators — split lowest precedence first.
    // `||` binds looser than `&&`, so split on `||` before `&&`.
    // We don't honor parentheses or short-circuit semantics beyond what
    // recursion gives us; `a || b || c` parses left-to-right via the
    // first `||` split, which matches common usage in givens.
    if let Some((lhs, rhs)) = split_top_level(expr, "||") {
        return evaluate_given(lhs, state, attrs, ctx) || evaluate_given(rhs, state, attrs, ctx);
    }
    if let Some((lhs, rhs)) = split_top_level(expr, "&&") {
        return evaluate_given(lhs, state, attrs, ctx) && evaluate_given(rhs, state, attrs, ctx);
    }

    // `field.any?` → field.size > 0; `field.empty?` → field.size == 0.
    // Ruby idioms used in handwritten bluebooks; rewrite to runtime
    // primitives so the comparison evaluator below handles them.
    if let Some(field) = expr.strip_suffix(".any?") {
        let val = resolve_expr(&format!("{}.size", field.trim()), state, attrs, ctx);
        return compare_lt(&Value::Int(0), &val);
    }
    if let Some(field) = expr.strip_suffix(".empty?") {
        let val = resolve_expr(&format!("{}.size", field.trim()), state, attrs, ctx);
        return values_equal(&val, &Value::Int(0));
    }

    // `list_field.include?(arg)` -> membership : true when the list field (a
    // list_of attr stored as Value::List, or a CSV Value::Str) contains the
    // resolved value of arg. Powers LOCAL list-membership gates, e.g.
    // Board.ActivateSprint's given { queued_sprints.include?(sprint_ref) }
    // (only a sprint already placed on the board may be activated). Mirrors
    // Ruby's native Array#include? so the same given evaluates identically in
    // both runtimes.
    if let Some(open) = expr.find(".include?(") {
        if let Some(without_close) = expr.strip_suffix(')') {
            let field = expr[..open].trim();
            let arg = without_close[open + ".include?(".len()..].trim();
            let haystack = attrs.get(field).cloned().unwrap_or_else(|| state.get(field).clone());
            let needle = resolve_expr(arg, state, attrs, ctx);
            let needle_s = match &needle {
                Value::Str(s) => s.clone(),
                other => other.to_string(),
            };
            return match haystack {
                Value::List(items) => items.iter().any(|v| v.to_string() == needle_s),
                Value::Str(s) => s.split(',').any(|item| item.trim() == needle_s),
                _ => false,
            };
        }
    }

    // Order matters: check `>=`/`<=` BEFORE `>`/`<` so the longer
    // operator wins. The split_comparison helpers also bail out on the
    // shorter operator when the longer is present.
    if let Some((lhs, rhs)) = split_comparison(expr, ">=") {
        let left = resolve_expr(lhs.trim(), state, attrs, ctx);
        let right = resolve_expr(rhs.trim(), state, attrs, ctx);
        return !compare_lt(&left, &right);
    }
    if let Some((lhs, rhs)) = split_comparison(expr, "<=") {
        let left = resolve_expr(lhs.trim(), state, attrs, ctx);
        let right = resolve_expr(rhs.trim(), state, attrs, ctx);
        return !compare_lt(&right, &left);
    }
    if let Some((lhs, rhs)) = split_comparison(expr, "<") {
        let left = resolve_expr(lhs.trim(), state, attrs, ctx);
        let right = resolve_expr(rhs.trim(), state, attrs, ctx);
        return compare_lt(&left, &right);
    }
    if let Some((lhs, rhs)) = split_comparison(expr, ">") {
        let left = resolve_expr(lhs.trim(), state, attrs, ctx);
        let right = resolve_expr(rhs.trim(), state, attrs, ctx);
        return compare_lt(&right, &left);
    }
    if let Some((lhs, rhs)) = split_comparison(expr, "==") {
        let left = resolve_expr(lhs.trim(), state, attrs, ctx);
        let right = resolve_expr(rhs.trim(), state, attrs, ctx);
        return values_equal(&left, &right);
    }
    if let Some((lhs, rhs)) = split_comparison(expr, "!=") {
        let left = resolve_expr(lhs.trim(), state, attrs, ctx);
        let right = resolve_expr(rhs.trim(), state, attrs, ctx);
        return !values_equal(&left, &right);
    }

    // A bare boolean expression with no comparison operator — e.g. a VO
    // predicate derivation `given { balance.covers?(amount) }`, or a plain
    // Bool field. Resolve it and honour a Bool result ; a non-boolean bare
    // given preserves the historical permissive default (passes), so this
    // only ADDS truthiness for expressions that actually resolve to a Bool.
    match resolve_expr(expr, state, attrs, ctx) {
        Value::Bool(b) => b,
        _ => true,
    }
}

pub fn resolve_mutation_value(
    value_expr: &str,
    attrs: &HashMap<String, Value>,
    state: &AggregateState,
) -> Value {
    let value_expr = value_expr.trim();

    // Hash-like: { name: :name, amount: :amount }
    if value_expr.starts_with('{') && value_expr.ends_with('}') {
        let inner = &value_expr[1..value_expr.len() - 1];
        let mut map = HashMap::new();
        for pair in inner.split(',') {
            let pair = pair.trim();
            if let Some(colon) = pair.find(':') {
                let key = pair[..colon].trim().trim_start_matches(':');
                let val_ref = pair[colon + 1..].trim().trim_start_matches(':');
                let val = attrs
                    .get(val_ref)
                    .cloned()
                    .unwrap_or(Value::Str(val_ref.to_string()));
                map.insert(key.to_string(), val);
            }
        }
        return Value::Map(map);
    }

    // `:now` and `seconds_since(:field)` used to resolve here — both
    // reached into the system clock from inside the domain (DDD
    // anti-pattern). They're gone. Time is infrastructure: the caller
    // (test, hecksagon adapter, app) provides the timestamp as a
    // command attribute. The lifecycle_validator catches any bluebook
    // that tries to bring them back.

    if let Some(field) = value_expr.strip_prefix(':') {
        // Try attrs first, then state fields
        return attrs.get(field)
            .or_else(|| state.fields.get(field))
            .cloned()
            .unwrap_or(Value::Null);
    }

    if let Ok(n) = value_expr.parse::<i64>() {
        return Value::Int(n);
    }

    if value_expr.starts_with('"') && value_expr.ends_with('"') {
        return Value::Str(value_expr[1..value_expr.len() - 1].to_string());
    }

    // List literal: `[]` → empty list, `[1, 2]` → list of resolved items.
    // Without this, `then_set :foo, to: []` previously stored the string
    // "[]", which broke `given { foo.empty? }` (length 2, not 0).
    if value_expr.starts_with('[') && value_expr.ends_with(']') {
        let inner = value_expr[1..value_expr.len() - 1].trim();
        if inner.is_empty() {
            return Value::List(vec![]);
        }
        let items: Vec<Value> = inner
            .split(',')
            .map(|item| resolve_mutation_value(item.trim(), attrs, state))
            .collect();
        return Value::List(items);
    }

    if value_expr == "true" { return Value::Bool(true); }
    if value_expr == "false" { return Value::Bool(false); }

    attrs
        .get(value_expr)
        .cloned()
        .unwrap_or(Value::Str(value_expr.to_string()))
}

#[cfg(test)]
mod tests {
    use super::{check_givens, EvalCtx};
    use crate::parser;
    use crate::runtime::AggregateState;
    use std::collections::HashMap;

    // The runtime half of DX Tier-0 #2 : a command whose then_set named an
    // unknown op (recorded by the parser as invalid_op, never panicked) must
    // be REFUSED before apply_mutations — applying the placeholder Set would
    // silently write the wrong value.
    #[test]
    fn check_givens_refuses_invalid_mutation_op() {
        let domain = parser::parse(
            r#"Hecks.bluebook "T" do
  aggregate "Thing" do
    description "t"
    attribute :items, list_of(Item)
    value_object "Item" do
      attribute :n, Integer
    end
    command "Do" do
      role "X"
      attribute :n, Integer
      then_set :items, bogus_op: { n: :n }
    end
  end
end"#,
        );
        let cmd = &domain.aggregates[0].commands[0];
        let state = AggregateState::new("x");
        let attrs = HashMap::new();
        let result = check_givens(cmd, &state, &attrs, EvalCtx::default());
        assert!(result.is_err(), "invalid-op mutation must refuse dispatch");
        let msg = format!("{:?}", result.unwrap_err());
        assert!(msg.contains("bogus_op"), "error must name the bad op, got: {}", msg);
    }

    #[test]
    fn check_givens_admits_valid_mutation_ops() {
        let domain = parser::parse(
            r#"Hecks.bluebook "T" do
  aggregate "Thing" do
    description "t"
    attribute :count, Integer
    command "Do" do
      role "X"
      then_set :count, increment: 1
    end
  end
end"#,
        );
        let cmd = &domain.aggregates[0].commands[0];
        let state = AggregateState::new("x");
        let attrs = HashMap::new();
        assert!(
            check_givens(cmd, &state, &attrs, EvalCtx::default()).is_ok(),
            "a well-formed then_set must not be refused by the invalid-op guard"
        );
    }
}
