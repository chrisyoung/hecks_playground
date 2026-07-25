//! interp_givens — the given gate : check_givens (every command `given`
//! evaluated against state + attrs, GivenFailed on the first false),
//! evaluate_predicate (the public wrapper the invariants module reuses —
//! an invariant IS a given checked on every command's resulting state),
//! and evaluate_given (the comparison/boolean evaluator over
//! interp_expr's resolver). `interpreter::check_givens` /
//! `interpreter::evaluate_predicate` stay valid through parent re-exports.
//!
//! Cask extracted VERBATIM from runtime/interpreter.rs (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/interp_givens.rs — kernel-floor
//!  interpreter, relocated verbatim from interpreter.rs blanket.]

use super::interp_expr::resolve_expr;
use super::interp_expr_ops::{compare_lt, split_comparison, split_top_level, values_equal};
use super::interpreter::EvalCtx;
use super::{AggregateState, RuntimeError, Value};
use crate::ir::Command;
use std::collections::HashMap;

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
