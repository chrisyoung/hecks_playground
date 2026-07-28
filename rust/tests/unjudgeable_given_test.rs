//! unjudgeable_given_test — a predicate the interpreter cannot READ refuses the
//! command and says so (given-gate card, finding 2, 2026-07-28).
//!
//! `evaluate_given` used to end `_ => true` — "the historical permissive
//! default". Every typo, every renamed attribute, every Ruby method outside the
//! interpreter's subset resolved to a non-Bool and was READ AS SATISFIED. The
//! rule looked like a rule and gated nothing ; negated, it refused everything.
//! Nothing in the suite noticed, because a rule that always passes is
//! indistinguishable from a rule that holds.
//!
//! Now the gate has three outcomes — satisfied, refused, unreadable — and the
//! third is a DEFECT report : no payload can satisfy a predicate nobody can
//! read, so it is not the caller's refusal to fix.

use std::collections::HashMap;
use storehouse::parser;
use storehouse::runtime::{Runtime, RuntimeError, Value};

const BLUEBOOK: &str = r#"Hecks.bluebook "UnreadableProbe" do
  aggregate "Probe" do
    identified_by :name
    attribute :name, ProbeName
    attribute :count, Integer, default: 0
    value_object "ProbeName" do
      attribute :value, String
    end
    command "Start" do
      role "System"
      attribute :name, ProbeName
      then_set :name, to: :name
      emits "Started"
    end
    command "TypoGuard" do
      role "System"
      attribute :name, ProbeName
      given("the author meant ready") { redy }
      emits "TypoGuarded"
    end
    command "UnknownMethodGuard" do
      role "System"
      attribute :name, ProbeName
      given("a method the floor does not speak") { name.upcase? }
      emits "UnknownGuarded"
    end
    command "ReadableGuard" do
      role "System"
      attribute :name, ProbeName
      given("a rule that can be read") { count > 0 }
      emits "ReadableGuarded"
    end
  end
end"#;

fn attrs(pairs: &[(&str, Value)]) -> HashMap<String, Value> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
}

fn booted() -> Runtime {
    let mut rt = Runtime::boot_with_data_dir(parser::parse(BLUEBOOK), None);
    rt.dispatch(
        "UnreadableProbe::Probe.Start",
        attrs(&[("name", Value::Str("p".to_string()))]),
    )
    .expect("Start");
    rt
}

fn name_p() -> HashMap<String, Value> {
    attrs(&[("name", Value::Str("p".to_string()))])
}

#[test]
fn a_typod_field_is_a_defect_not_a_satisfied_rule() {
    let mut rt = booted();
    let outcome = rt.dispatch("UnreadableProbe::Probe.TypoGuard", name_p());
    match outcome {
        Err(RuntimeError::UnjudgeableGiven { clause, .. }) => assert_eq!(
            clause, "redy",
            "the refusal must name the clause it could not read"
        ),
        other => panic!("a typo'd bare name must be unjudgeable, got {other:?}"),
    }
}

// NOTE, and it is the honest limit of this fix : a typo INSIDE a comparison
// (`given { conut > 0 }`) is still not caught. `resolve_expr` spells an unknown
// bare name back as a Str, so the comparison reads `"conut" > 0`, coerces to 0,
// and answers false — a silent refusal wearing an author's message. That is the
// self-spelling fallback, finding 3's territory, and it is a separate flip.
// Only BARE leaves reach the arm this test covers.

#[test]
fn a_method_outside_the_subset_is_a_defect() {
    let mut rt = booted();
    let outcome = rt.dispatch("UnreadableProbe::Probe.UnknownMethodGuard", name_p());
    assert!(
        matches!(outcome, Err(RuntimeError::UnjudgeableGiven { .. })),
        "a Ruby method the interpreter does not implement must be unjudgeable, got {outcome:?}"
    );
}

#[test]
fn a_readable_rule_still_refuses_and_admits_normally() {
    let mut rt = booted();

    // count defaults to 0 — the rule is READ and NOT satisfied. Still a plain
    // GivenFailed, carrying the author's own message.
    let refused = rt.dispatch("UnreadableProbe::Probe.ReadableGuard", name_p());
    match refused {
        Err(RuntimeError::GivenFailed { message, .. }) => {
            assert_eq!(message, "a rule that can be read");
        }
        other => panic!("a readable, unsatisfied rule must be GivenFailed, got {other:?}"),
    }

    // And it admits when satisfied — the flip did not make every given refuse.
    let mut with_count = name_p();
    with_count.insert("count".to_string(), Value::Int(3));
    rt.dispatch("UnreadableProbe::Probe.ReadableGuard", with_count)
        .expect("a satisfied rule must still admit");
}
