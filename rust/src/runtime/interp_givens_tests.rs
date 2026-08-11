//! interp_givens_tests — the given-gate suite : negation semantics (binds
//! tighter than binary ops, composes with any, !empty? refuses blanks)
//! and %w[] word-array membership — the parity-arc behaviors landed
//! 2026-07-25.
//!
//! Cask extracted VERBATIM from runtime/interp_givens.rs (cask-runtime) ;
//! body dedented one level out of the inline mod.
//!
//! [antibody-exempt: rust/src/runtime/interp_givens_tests.rs —
//!  kernel-floor given tests, relocated verbatim from interp_givens.rs
//!  blanket.]

use super::interp_predicate::evaluate_predicate;
use super::interpreter::EvalCtx;
use super::{AggregateState, Value};
use std::collections::HashMap;

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
