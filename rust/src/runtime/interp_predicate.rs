//! interp_predicate — the predicate evaluator : evaluate_predicate (f4,
//! the public wrapper the invariants module reuses — an invariant IS a
//! given) and evaluate_given (comparison/boolean evaluation with negation
//! binding tighter than binary ops, any/all composition, %w[] word-array
//! membership). The command given-loop (check_givens) stays in
//! interp_givens.rs.
//!
//! Cask extracted VERBATIM from runtime/interp_givens.rs (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/interp_predicate.rs — kernel-floor
//!  interpreter, relocated verbatim from interp_givens.rs blanket.]

use super::interp_expr::resolve_expr;
use super::interp_expr_ops::{compare_lt, split_comparison, split_top_level, values_equal};
use super::interpreter::EvalCtx;
use super::{AggregateState, Value};
use std::collections::HashMap;

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
    // The `!expr.starts_with('!')` guard is what keeps NEGATION working. These
    // two branches match on a SUFFIX, so without the guard `!value.empty?`
    // matches here and strips `.empty?` off `!value` rather than off `value` —
    // the `!` is silently discarded, the unknown field `!value` resolves to 0,
    // `0 == 0` is true, and the rule passes. Every `requires { !value.empty? }`
    // in the corpus (storehouse/lexicon, story, command_bus ; adapters heki,
    // ollama, r2, storehouse_api) read "must not be blank" and never fired.
    // Negation is handled below, at its correct precedence.
    if !expr.starts_with('!') {
        // `field.nil?` — Ruby's absence test. Three corpus givens read
        // `!title.nil? && !title.empty?` ; unimplemented, `title.nil?` fell all
        // the way to the bare arm, passed by the permissive default, and then
        // NEGATED to a permanent false — so Feature.PlanAdditions,
        // Feature.VerifyAdditions and Notification.DismissAlert could not be
        // dispatched at all, with any payload. The permissive default does not
        // only fail open ; under `!` it fails CLOSED, and neither is a verdict
        // the author wrote.
        //
        // Resolved by LOOKUP, not resolve_expr : an unknown bare name resolves
        // to its own spelling there, which is never Null, so every absent field
        // would answer "not nil". A DOTTED path has no flat entry to find, so it
        // resolves — the same asymmetry the `.size` receiver makes.
        if let Some(field) = expr.strip_suffix(".nil?") {
            let f = field.trim();
            let val = if f.contains('.') {
                resolve_expr(f, state, attrs, ctx)
            } else {
                attrs.get(f).cloned().unwrap_or_else(|| state.get(f).clone())
            };
            return matches!(val, Value::Null);
        }
        if let Some(field) = expr.strip_suffix(".any?") {
            let val = resolve_expr(&format!("{}.size", field.trim()), state, attrs, ctx);
            return compare_lt(&Value::Int(0), &val);
        }
        if let Some(field) = expr.strip_suffix(".empty?") {
            let val = resolve_expr(&format!("{}.size", field.trim()), state, attrs, ctx);
            return values_equal(&val, &Value::Int(0));
        }
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
            // A LITERAL receiver resolves ; a FIELD receiver is looked up as
            // before. Only the literal form goes through resolve_expr, because
            // an unresolvable bare name there falls through to Str(name) and a
            // field haystack would silently become its own spelling.
            let haystack = if field.starts_with("%w[") {
                resolve_expr(field, state, attrs, ctx)
            } else {
                attrs.get(field).cloned().unwrap_or_else(|| state.get(field).clone())
            };
            let needle = resolve_expr(arg, state, attrs, ctx);
            let needle_s = match &needle {
                Value::Str(s) => s.clone(),
                other => other.to_string(),
            };
            return match haystack {
                Value::List(items) => items.iter().any(|v| v.to_string() == needle_s),
                // RUBY'S MEANING, which is SUBSTRING. This read the string as
                // comma-separated fields and asked whether any WHOLE one
                // equalled the needle, so `address.include?("@")` answered
                // false for "ada@example.com" — refusing every address that
                // was in fact an address. The comment above still claims this
                // branch mirrors Ruby ; it mirrored Array#include?, never
                // String#include?. A list that happens to be stored as text is
                // a LIST, and bending String#include? to serve that storage
                // choice broke the claim for every genuine string.
                Value::Str(s) => s.contains(needle_s.as_str()),
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

    // NEGATION — `!expr`. Sits AFTER the comparison splits because `!` binds
    // TIGHTER than every binary operator: Ruby reads `!a && b` as `(!a) && b`,
    // so reaching negation earlier would read it as `!(a && b)` and invert the
    // verdict on a sentence that looks unambiguous. `a != b` never arrives here
    // — it does not START with `!`, and `!=` is split above.
    //
    // Mirrors lib/hecksagain/bluebook/expression/evaluator.rb, which places the
    // same branch in the same position for the same reason.
    if let Some(inner) = expr.strip_prefix('!') {
        return !evaluate_given(inner.trim(), state, attrs, ctx);
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
