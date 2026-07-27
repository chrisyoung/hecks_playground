//! interp_givens — the given gate : check_givens (every command `given`
//! evaluated against state + attrs, GivenFailed on the first false),
//! evaluate_predicate (the public wrapper the invariants module reuses —
//! an invariant IS a given checked on every command's resulting state),
//! and evaluate_given (the comparison/boolean evaluator over
//! interp_expr's resolver). `interpreter::check_givens` /
//! `interpreter::evaluate_predicate` stay valid through parent re-exports.
//!
//! SPLIT ORDER, lowest precedence outward — this is semantics, not style :
//!
//!   ||  ->  &&  ->  .any? / .empty?  ->  .include?  ->  >= <= < > == !=
//!       ->  !  ->  bare
//!
//! `!` sits second-from-last because it binds TIGHTER than every binary
//! operator: Ruby reads `!a && b` as `(!a) && b`. The `.any?` / `.empty?`
//! branches match on a SUFFIX, so they carry an explicit guard against a
//! leading `!` — without it they strip the suffix off `!value` instead of
//! `value` and swallow the negation entirely.
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
    // The `!expr.starts_with('!')` guard is what keeps NEGATION working. These
    // two branches match on a SUFFIX, so without the guard `!value.empty?`
    // matches here and strips `.empty?` off `!value` rather than off `value` —
    // the `!` is silently discarded, the unknown field `!value` resolves to 0,
    // `0 == 0` is true, and the rule passes. Every `requires { !value.empty? }`
    // in the corpus (storehouse/lexicon, story, command_bus ; adapters heki,
    // ollama, r2, storehouse_api) read "must not be blank" and never fired.
    // Negation is handled below, at its correct precedence.
    if !expr.starts_with('!') {
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

#[cfg(test)]
mod tests {
    use super::evaluate_predicate;
    use super::{AggregateState, EvalCtx, HashMap, Value};

    fn with_value(text: &str) -> AggregateState {
        let mut state = AggregateState::new("x");
        state.set("value", Value::Str(text.to_string()));
        state
    }

    fn check(expr: &str, state: &AggregateState) -> bool {
        evaluate_predicate(expr, state, &HashMap::new(), EvalCtx::default())
    }

    /// THE REGRESSION. A predicate and its exact negation must disagree.
    ///
    /// `.empty?` matches on a SUFFIX, so before the guard above it stripped
    /// `.empty?` off `!value` rather than off `value` — the `!` was silently
    /// discarded, the unknown field `!value` resolved to 0, `0 == 0` was true,
    /// and BOTH of these answered true. Every `requires { !value.empty? }` in
    /// the corpus read "must not be blank" and never fired.
    #[test]
    fn negation_disagrees_with_the_predicate_it_negates() {
        let state = with_value("Hello");
        assert!(!check("value.empty?", &state));
        assert!(check("!value.empty?", &state));
    }

    /// The corpus rule doing its job: a blank value is refused.
    #[test]
    fn negated_empty_refuses_a_blank_value() {
        let state = with_value("");
        assert!(check("value.empty?", &state));
        assert!(!check("!value.empty?", &state));
    }

    /// `!` binds TIGHTER than the binary operators: `!a && b` is `(!a) && b`,
    /// never `!(a && b)`.
    #[test]
    fn negation_binds_tighter_than_the_binary_operators() {
        let mut state = AggregateState::new("x");
        state.set("ready", Value::Bool(false));
        state.set("open", Value::Bool(true));
        assert!(check("!ready && open", &state));

        state.set("ready", Value::Bool(true));
        assert!(!check("!ready && open", &state));
    }

    /// `.any?` is the mirror of `.empty?` and took the same suffix-match fault.
    #[test]
    fn negation_composes_with_any() {
        let state = with_value("Hello");
        assert!(check("value.any?", &state));
        assert!(!check("!value.any?", &state));
    }

    // WORD-ARRAY MEMBERSHIP — the corpus's way of saying "one of these", and
    // 58 invariants across 14 bluebooks are written this way. Until the list
    // literal existed NONE fired : the receiver resolved to nothing, include?
    // fell to its false arm, and the payload gate skipped the clause as
    // unjudgeable. Every one looked exactly like a rule.

    const MODE: &str = "%w[ensure status stop].include?(value)";

    fn judge(expression: &str, value: &str) -> bool {
        let mut attrs: HashMap<String, Value> = HashMap::new();
        attrs.insert("value".to_string(), Value::Str(value.to_string()));

        evaluate_predicate(expression, &AggregateState::new("x"), &attrs, EvalCtx::default())
    }

    #[test]
    fn word_array_admits_a_member() {
        assert!(judge(MODE, "status"));
        assert!(judge(MODE, "ensure"));
        assert!(judge(MODE, "stop"));
    }

    #[test]
    fn word_array_refuses_a_non_member() {
        assert!(!judge(MODE, "banana"));
        assert!(!judge(MODE, ""));
        // A near-miss is still a miss : membership is over WHOLE words.
        assert!(!judge(MODE, "statuses"));
    }

    /// String include? is SUBSTRING, as in Ruby. This branch read the string as
    /// comma-separated fields and asked whether any WHOLE one equalled the
    /// needle, so an address containing an @ was reported as not containing one.
    #[test]
    fn string_include_is_substring_as_in_ruby() {
        let mut attrs: HashMap<String, Value> = HashMap::new();
        attrs.insert("address".to_string(), Value::Str("ada@example.com".to_string()));

        let judge_address = |expr: &str| {
            evaluate_predicate(expr, &AggregateState::new("x"), &attrs, EvalCtx::default())
        };

        assert!(judge_address("address.include?(\"@\")"));
        assert!(!judge_address("address.include?(\"!\")"));
    }

    /// A LIST field still answers membership, not substring — the arm that was
    /// already correct, pinned so the substring fix cannot quietly take it over.
    #[test]
    fn a_list_field_still_answers_membership() {
        let mut attrs: HashMap<String, Value> = HashMap::new();
        attrs.insert(
            "queued".to_string(),
            Value::List(vec![Value::Str("a".into()), Value::Str("b".into())]),
        );

        let judge_queued = |expr: &str| {
            evaluate_predicate(expr, &AggregateState::new("x"), &attrs, EvalCtx::default())
        };

        assert!(judge_queued("queued.include?(\"a\")"));
        assert!(!judge_queued("queued.include?(\"c\")"));
    }
}
