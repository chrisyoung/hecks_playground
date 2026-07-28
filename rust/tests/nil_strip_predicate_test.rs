//! nil_strip_predicate_test — `.nil?` and `.strip` are words the corpus
//! speaks and the interpreter did not (found by the bare-arm audit,
//! 2026-07-28).
//!
//! Four corpus commands guard themselves the Ruby way :
//!
//!     Feature.PlanAdditions      given { !title.nil? && !title.empty? }
//!     Feature.VerifyAdditions    given { !additions.nil? && !additions.empty? }
//!     Notification.DismissAlert  given { !alerts.nil? && alerts.size > alert_index }
//!     Search.SearchDomain        given { !query_text.strip.empty? }
//!
//! Neither word was implemented. `title.nil?` fell to the bare arm, passed by
//! the permissive default, and NEGATED to a permanent false ; `query_text.strip`
//! was looked up as a flat field name, missed, reported size 0, and `.empty?`
//! answered true. All four commands were UN-DISPATCHABLE with any payload — the
//! permissive default fails open, and under `!` it fails closed. Neither verdict
//! is the one the author wrote.
//!
//! Every rule here is exercised in BOTH directions. A rule that cannot be made
//! to fail is not a rule ; a rule that cannot be made to pass is not one either.

use std::collections::HashMap;
use storehouse::parser;
use storehouse::runtime::{Runtime, Value};

const BLUEBOOK: &str = r#"Hecks.bluebook "GuardProbe" do
  aggregate "Probe" do
    identified_by :name
    attribute :name, ProbeName
    attribute :title, Title
    value_object "ProbeName" do
      attribute :value, String
    end
    value_object "Title" do
      attribute :value, String
    end
    command "Start" do
      role "System"
      attribute :name, ProbeName
      then_set :name, to: :name
      emits "Started"
    end
    command "NilGuard" do
      role "System"
      attribute :name, ProbeName
      attribute :title, Title
      given("title present") { !title.nil? && !title.empty? }
      then_set :title, to: :title
      emits "NilGuarded"
    end
    command "StripGuard" do
      role "System"
      attribute :name, ProbeName
      attribute :title, Title
      given("title not blank") { !query_text.strip.empty? }
      then_set :title, to: :title
      emits "StripGuarded"
    end
  end
end"#;

fn s(v: &str) -> Value {
    Value::Str(v.to_string())
}

fn attrs(pairs: &[(&str, Value)]) -> HashMap<String, Value> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
}

fn booted() -> Runtime {
    let mut rt = Runtime::boot_with_data_dir(parser::parse(BLUEBOOK), None);
    rt.dispatch("GuardProbe::Probe.Start", attrs(&[("name", s("p"))]))
        .expect("Start");
    rt
}

#[test]
fn a_present_value_is_not_nil_and_an_absent_one_is() {
    let mut rt = booted();

    // PRESENT — the command must dispatch. Before `.nil?` existed this was a
    // permanent GivenFailed : `!title.nil?` was always false.
    rt.dispatch(
        "GuardProbe::Probe.NilGuard",
        attrs(&[("name", s("p")), ("title", s("A real title"))]),
    )
    .expect("a present title must satisfy `!title.nil? && !title.empty?`");

    // EMPTY — still refused, by the `.empty?` half.
    let refused = rt.dispatch(
        "GuardProbe::Probe.NilGuard",
        attrs(&[("name", s("p")), ("title", s(""))]),
    );
    assert!(refused.is_err(), "an empty title must be refused, got {refused:?}");

    // ABSENT — refused by the `.nil?` half : the attribute never arrived AND the
    // record carries no title. A FRESH runtime, deliberately : on the record
    // above the first dispatch already wrote `title`, and a given reads state
    // when the payload is silent, so re-using it would test the wrong absence.
    let mut fresh = booted();
    let refused = fresh.dispatch("GuardProbe::Probe.NilGuard", attrs(&[("name", s("p"))]));
    assert!(refused.is_err(), "an absent title must be refused, got {refused:?}");
}

#[test]
fn strip_trims_before_the_blank_test() {
    let mut rt = booted();

    // PRESENT — dispatches. Before `.strip` existed, `query_text.strip` was a
    // flat field name that missed, reported size 0, and refused everything.
    rt.dispatch(
        "GuardProbe::Probe.StripGuard",
        attrs(&[("name", s("p")), ("title", s("t")), ("query_text", s("hello"))]),
    )
    .expect("a non-blank query must satisfy `!query_text.strip.empty?`");

    // WHITESPACE-ONLY — refused. This is the whole point of writing `.strip` :
    // "   " is not blank to `.empty?` and IS blank to a reader.
    let refused = rt.dispatch(
        "GuardProbe::Probe.StripGuard",
        attrs(&[("name", s("p")), ("title", s("t")), ("query_text", s("   "))]),
    );
    assert!(refused.is_err(), "a whitespace-only query must be refused, got {refused:?}");
}
