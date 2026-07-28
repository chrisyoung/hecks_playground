//! Computed identity : an id DERIVED from declared facts, not minted.
//!
//! DDD says identity is what does not change. A counter-minted id changes with
//! insertion order ; a UUID changes every run. Both make it impossible for a
//! hecksagon to NAME the record it means — which is exactly why a pizzeria's
//! bank account could not be addressed as anything but "1".
//!
//! The FQN is deliberately NOT part of it : the aggregate name is the thing we
//! rename (Takings -> Deposit, the same night this was written), and an identity
//! that moves when the model is renamed is not an identity.

use std::collections::HashMap;
use storehouse::runtime::{Runtime, RuntimeError, Value};
use storehouse::parser;

fn s(v: &str) -> Value {
    Value::Str(v.to_string())
}

fn attrs(pairs: &[(&str, Value)]) -> HashMap<String, Value> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
}

const BLUEBOOK: &str = r#"Hecks.bluebook "Bank" do
  aggregate "Account" do
    identified_by do
      customer
      number
    end
    attribute :customer, String
    attribute :number, String
    attribute :balance, Integer, default: 0
    command "Open" do
      role "Teller"
      attribute :customer, String
      attribute :number, String
      then_set :customer, to: :customer
      then_set :number, to: :number
      emits "AccountOpened"
    end
    command "Deposit" do
      role "Teller"
      attribute :customer, String
      attribute :number, String
      attribute :amount, Integer
      then_set :balance, increment: :amount
      emits "Deposited"
    end
  end

  aggregate "Lexicon" do
    identified_by do
      "lexicon"
    end
    attribute :phrase_count, Integer, default: 0
    command "Compile" do
      role "System"
      attribute :phrase_count, Integer
      then_set :phrase_count, to: :phrase_count
      emits "LexiconCompiled"
    end
  end
end"#;

fn booted() -> Runtime {
    Runtime::boot_with_data_dir(parser::parse(BLUEBOOK), None)
}

#[test]
fn the_id_is_derived_from_the_declared_parts() {
    let mut rt = booted();
    rt.dispatch(
        "Bank::Account.Open",
        attrs(&[("customer", s("ada")), ("number", s("4021"))]),
    )
    .expect("open");

    let ids: Vec<String> = rt.all("Account").iter().map(|r| r.id.clone()).collect();
    assert_eq!(ids, vec!["ada:4021"], "the id IS the declared facts, joined");
}

#[test]
fn the_same_facts_name_the_same_record_every_time() {
    let mut rt = booted();
    for _ in 0..3 {
        rt.dispatch(
            "Bank::Account.Open",
            attrs(&[("customer", s("ada")), ("number", s("4021"))]),
        )
        .expect("open");
    }
    assert_eq!(
        rt.all("Account").len(),
        1,
        "three opens of the same account are ONE account, not three"
    );

    // And a later command finds it without being told an id.
    rt.dispatch(
        "Bank::Account.Deposit",
        attrs(&[("customer", s("ada")), ("number", s("4021")), ("amount", Value::Int(1400))]),
    )
    .expect("deposit");

    let balance = rt.all("Account")[0].fields.get("balance").map(|v| v.to_string());
    assert_eq!(balance.as_deref(), Some("1400"), "it deposited into the same account");
}

#[test]
fn different_facts_are_different_records() {
    let mut rt = booted();
    rt.dispatch("Bank::Account.Open", attrs(&[("customer", s("ada")), ("number", s("4021"))]))
        .expect("open");
    rt.dispatch("Bank::Account.Open", attrs(&[("customer", s("grace")), ("number", s("4021"))]))
        .expect("open");

    let mut ids: Vec<String> = rt.all("Account").iter().map(|r| r.id.clone()).collect();
    ids.sort();
    assert_eq!(ids, vec!["ada:4021", "grace:4021"], "same number, different customer");
}

/// An identity fact is never CHANGED, because it cannot be : the id is a
/// function of the facts, so different facts name a different entity. There is
/// no operation that moves `ada:4021` to `ada:4099` — the second is simply
/// somebody else. That is what makes derived identity safe to reference from an
/// immutable event log : nothing it points at can ever move out from under it.
///
/// The hazard it leaves is the opposite one, pinned by the next test : a TYPO in
/// an identity fact does not fail, it addresses a record that does not exist.
#[test]
fn different_identity_facts_are_a_different_entity_not_a_moved_one() {
    let mut rt = booted();
    rt.dispatch(
        "Bank::Account.Open",
        attrs(&[("customer", s("ada")), ("number", s("4021"))]),
    )
    .expect("open");
    rt.dispatch(
        "Bank::Account.Deposit",
        attrs(&[("customer", s("ada")), ("number", s("4021")), ("amount", Value::Int(1400))]),
    )
    .expect("deposit into the account that exists");

    // A different number is a different account, and the original is untouched.
    rt.dispatch(
        "Bank::Account.Deposit",
        attrs(&[("customer", s("ada")), ("number", s("4099")), ("amount", Value::Int(100))]),
    )
    .expect("dispatch");

    let original = rt.all("Account").into_iter().find(|r| r.id == "ada:4021").expect("4021");
    assert_eq!(
        original.fields.get("balance").map(|v| v.to_string()).as_deref(),
        Some("1400"),
        "the original account must be untouched by a command naming other facts"
    );
}

#[test]
fn re_stating_the_same_identity_facts_is_not_a_change() {
    let mut rt = booted();
    rt.dispatch(
        "Bank::Account.Open",
        attrs(&[("customer", s("ada")), ("number", s("4021"))]),
    )
    .expect("open");

    // Every later command carries the identity facts — that is HOW it finds the
    // record. Saying the same thing again must never read as moving it.
    rt.dispatch(
        "Bank::Account.Deposit",
        attrs(&[
            ("customer", s("ada")),
            ("number", s("4021")),
            ("amount", Value::Int(1400)),
        ]),
    )
    .expect("the same facts are not a change");

    assert_eq!(
        rt.all("Account")[0].fields.get("balance").map(|v| v.to_string()).as_deref(),
        Some("1400")
    );
}

#[test]
fn a_singleton_says_there_is_one_of_it_with_a_literal() {
    let mut rt = booted();
    rt.dispatch("Bank::Lexicon.Compile", attrs(&[("phrase_count", Value::Int(7))]))
        .expect("compile");
    rt.dispatch("Bank::Lexicon.Compile", attrs(&[("phrase_count", Value::Int(9))]))
        .expect("compile again");

    let rows = rt.all("Lexicon");
    assert_eq!(rows.len(), 1, "a singleton is one row, however often it runs");
    assert_eq!(rows[0].id, "lexicon", "and it is named, not numbered");
    assert_eq!(
        rows[0].fields.get("phrase_count").map(|v| v.to_string()).as_deref(),
        Some("9"),
        "the second compile updated the same record"
    );
}
