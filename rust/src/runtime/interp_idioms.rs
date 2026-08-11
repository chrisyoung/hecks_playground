//! interp_idioms — the Ruby idioms handwritten bluebooks reach for, judged
//! before the comparison ladder : `.nil?` (absence), `.any?` / `.empty?`
//! (rewritten to `.size` comparisons), and `.include?(arg)` (membership over a
//! list field or a %w[] literal).
//!
//! They live together because they share one rule : each matches on a SUFFIX,
//! so a leading `!` must be refused here and handled at its own precedence in
//! interp_predicate.rs — otherwise `!value.empty?` strips the suffix off
//! `!value` and swallows the negation. That guard is semantics, not style.
//!
//! `Some(verdict)` when an idiom claims the expression, `None` when none does.
//! No idiom is ever UNJUDGEABLE : each asks a question about presence or
//! membership that has a true/false answer even when the receiver is absent.
//!
//! Cask extracted from runtime/interp_predicate.rs (cask-runtime).

use super::interp_expr::resolve_expr;
use super::interp_expr_ops::{compare_lt, values_equal};
use super::interpreter::EvalCtx;
use super::{AggregateState, Value};
use std::collections::HashMap;

pub(super) fn judge_idiom(
    expr: &str,
    state: &AggregateState,
    attrs: &HashMap<String, Value>,
    ctx: EvalCtx,
) -> Option<bool> {
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
        return Some(matches!(val, Value::Null));
    }
    if let Some(field) = expr.strip_suffix(".any?") {
        let val = resolve_expr(&format!("{}.size", field.trim()), state, attrs, ctx);
        return Some(compare_lt(&Value::Int(0), &val));
    }
    if let Some(field) = expr.strip_suffix(".empty?") {
        let val = resolve_expr(&format!("{}.size", field.trim()), state, attrs, ctx);
        return Some(values_equal(&val, &Value::Int(0)));
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
        return Some(match haystack {
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
        });
    }
}
    None
}
