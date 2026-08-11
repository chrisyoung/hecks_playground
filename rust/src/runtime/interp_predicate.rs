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
use super::interp_idioms::judge_idiom;
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

/// A predicate the interpreter could not READ — it resolved to something that
/// is neither true nor false. Not a false : an unknown name, a typo'd
/// attribute, or a Ruby method outside the interpreter's subset is a DEFECT in
/// the rule, and a defect deserves a different verdict from "the rule was read
/// and not satisfied". `clause` is the leaf that could not be read, which is
/// rarely the whole expression.
pub(super) struct Unjudgeable {
    pub clause: String,
}

/// The total wrapper, for the callers with no error channel : aggregate
/// invariants, the payload gate, and a derivation body evaluated inside
/// `resolve_expr`. An unreadable predicate is FALSE for them — refused loudly
/// rather than passed silently. `check_givens` calls `judge` directly so a
/// command's refusal can name the clause it could not read.
pub(super) fn evaluate_given(
    expr: &str,
    state: &AggregateState,
    attrs: &HashMap<String, Value>,
    ctx: EvalCtx,
) -> bool {
    judge(expr, state, attrs, ctx).unwrap_or(false)
}

pub(super) fn judge(
    expr: &str,
    state: &AggregateState,
    attrs: &HashMap<String, Value>,
    ctx: EvalCtx,
) -> Result<bool, Unjudgeable> {
    let expr = expr.trim();

    // Boolean operators — split lowest precedence first.
    // `||` binds looser than `&&`, so split on `||` before `&&`.
    // We don't honor parentheses or short-circuit semantics beyond what
    // recursion gives us; `a || b || c` parses left-to-right via the
    // first `||` split, which matches common usage in givens.
    // Both clauses are judged even when the first decides the verdict : an
    // unreadable clause is a defect wherever it sits, and a short-circuit would
    // hide exactly the half nobody exercises. Ruby's own short-circuit is about
    // side effects ; a given has none.
    if let Some((lhs, rhs)) = split_top_level(expr, "||") {
        let (l, r) = (judge(lhs, state, attrs, ctx)?, judge(rhs, state, attrs, ctx)?);
        return Ok(l || r);
    }
    if let Some((lhs, rhs)) = split_top_level(expr, "&&") {
        let (l, r) = (judge(lhs, state, attrs, ctx)?, judge(rhs, state, attrs, ctx)?);
        return Ok(l && r);
    }

    // Ruby idioms the corpus writes — `.nil?`, `.any?`, `.empty?`, `.include?`.
    // Their branch order and the leading-`!` guard are SEMANTICS, not style ;
    // both live with them in interp_idioms.rs. None of them can be unjudgeable :
    // each one asks a question about presence or membership that always has a
    // true/false answer, even when the receiver is absent.
    if let Some(verdict) = judge_idiom(expr, state, attrs, ctx) {
        return Ok(verdict);
    }

    // Order matters: check `>=`/`<=` BEFORE `>`/`<` so the longer
    // operator wins. The split_comparison helpers also bail out on the
    // shorter operator when the longer is present.
    if let Some((lhs, rhs)) = split_comparison(expr, ">=") {
        let left = resolve_expr(lhs.trim(), state, attrs, ctx);
        let right = resolve_expr(rhs.trim(), state, attrs, ctx);
        return Ok(!compare_lt(&left, &right));
    }
    if let Some((lhs, rhs)) = split_comparison(expr, "<=") {
        let left = resolve_expr(lhs.trim(), state, attrs, ctx);
        let right = resolve_expr(rhs.trim(), state, attrs, ctx);
        return Ok(!compare_lt(&right, &left));
    }
    if let Some((lhs, rhs)) = split_comparison(expr, "<") {
        let left = resolve_expr(lhs.trim(), state, attrs, ctx);
        let right = resolve_expr(rhs.trim(), state, attrs, ctx);
        return Ok(compare_lt(&left, &right));
    }
    if let Some((lhs, rhs)) = split_comparison(expr, ">") {
        let left = resolve_expr(lhs.trim(), state, attrs, ctx);
        let right = resolve_expr(rhs.trim(), state, attrs, ctx);
        return Ok(compare_lt(&right, &left));
    }
    if let Some((lhs, rhs)) = split_comparison(expr, "==") {
        let left = resolve_expr(lhs.trim(), state, attrs, ctx);
        let right = resolve_expr(rhs.trim(), state, attrs, ctx);
        return Ok(values_equal(&left, &right));
    }
    if let Some((lhs, rhs)) = split_comparison(expr, "!=") {
        let left = resolve_expr(lhs.trim(), state, attrs, ctx);
        let right = resolve_expr(rhs.trim(), state, attrs, ctx);
        return Ok(!values_equal(&left, &right));
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
        return Ok(!judge(inner.trim(), state, attrs, ctx)?);
    }

    // A bare boolean expression with no comparison operator — a VO predicate
    // derivation (`given { balance.covers?(amount) }`), or a plain Bool field.
    // Resolve it and honour a Bool result.
    //
    // Anything else is UNJUDGEABLE. This was `_ => true`, "the historical
    // permissive default", and it decided every typo, every renamed attribute,
    // and every Ruby method outside this subset — silently, in the author's
    // name. It reads as a satisfied rule, and under a leading `!` (how the
    // corpus writes nearly every presence check) it inverts into a permanent
    // refusal : `.nil?` and `.strip` were missing for exactly that long, and
    // four commands could not be dispatched with any payload at all. A
    // predicate that cannot be evaluated is a defect, not a false.
    match resolve_expr(expr, state, attrs, ctx) {
        Value::Bool(b) => Ok(b),
        _ => Err(Unjudgeable { clause: expr.to_string() }),
    }
}
