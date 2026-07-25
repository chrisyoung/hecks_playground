//! interp_mutations — apply_mutations, the then_set executor : set / append
//! / increment / decrement (+ VO arithmetic via vo_arith), clamp bounds
//! (clamp_bounds, field_numeric), list ops. The mutation VALUE resolver
//! (resolve_mutation_value) and the expression machinery live in
//! interpreter.rs / interp_expr.rs.
//!
//! Cask extracted VERBATIM from runtime/interpreter.rs (cask-runtime) ;
//! `interpreter::apply_mutations` stays valid through the parent re-export.
//!
//! [antibody-exempt: rust/src/runtime/interp_mutations.rs — kernel-floor
//!  interpreter, relocated verbatim from interpreter.rs blanket.]

use super::interp_expr_ops::numeric_value;
use super::interpreter::resolve_mutation_value;
use super::{AggregateState, Value};
use crate::ir::{Command, MutationOp};
use std::collections::HashMap;

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
