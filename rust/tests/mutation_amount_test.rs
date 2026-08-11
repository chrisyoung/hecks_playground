//! mutation_amount_test.rs — regression for the ±1 fallback (found via the
//! banking-exact audit, 2026-07-27) : `then_set :field, increment:/decrement:`
//! with an amount that will not read as a number used `as_int().unwrap_or(1)`
//! and moved the field by one. A deposit of a mis-shaped payload moved a
//! banking balance by one cent — silently, in production shape. The mutation
//! now SKIPS LOUDLY (stderr) and the field does not move. (hecksagain's
//! runtime refuses the whole dispatch ; growing apply() a Result to match is
//! the named follow-up on DESIGN-banking-exact.)
//!
//! [antibody-exempt: rust/tests/mutation_amount_test.rs — kernel-surface
//!  regression test asserting Rust runtime state ; same category as
//!  list_vo_default_test.rs.]

use std::collections::HashMap;
use storehouse::parser;
use storehouse::runtime::{Runtime, Value};

const BLUEBOOK: &str = r#"Hecks.bluebook "CountProbe" do
  aggregate "Counter" do
    identified_by :name
    attribute :name, CounterName
    attribute :total, Integer, default: 0
    value_object "CounterName" do
      attribute :value, String
    end
    command "Start" do
      role "System"
      attribute :name, CounterName
      then_set :name, to: :name
      emits "Started"
    end
    command "Add" do
      role "System"
      attribute :amount, Integer
      then_set :total, increment: :amount
      emits "Added"
    end
    command "Take" do
      role "System"
      attribute :amount, Integer
      then_set :total, decrement: :amount
      emits "Took"
    end
  end
end"#;

fn total(rt: &Runtime) -> Value {
    rt.all("Counter")[0].get("total").clone()
}

#[test]
fn a_non_numeric_amount_moves_nothing() {
    let domain = parser::parse(BLUEBOOK);
    let mut rt = Runtime::boot_with_data_dir(domain, None);

    let mut attrs = HashMap::new();
    attrs.insert("name".to_string(), Value::Str("c".to_string()));
    rt.dispatch("CountProbe::Counter.Start", attrs).expect("Start");

    let mut attrs = HashMap::new();
    attrs.insert("name".to_string(), Value::Str("c".to_string()));
    attrs.insert("amount".to_string(), Value::Int(250));
    rt.dispatch("CountProbe::Counter.Add", attrs).expect("Add");
    assert_eq!(total(&rt), Value::Int(250));

    // "lots" is not a number. The old fallback made this 251 — a ledger
    // drifting by one cent per bad payload.
    let mut attrs = HashMap::new();
    attrs.insert("name".to_string(), Value::Str("c".to_string()));
    attrs.insert("amount".to_string(), Value::Str("lots".to_string()));
    rt.dispatch("CountProbe::Counter.Add", attrs).expect("Add dispatches");
    assert_eq!(total(&rt), Value::Int(250), "a non-numeric increment must move NOTHING");

    let mut attrs = HashMap::new();
    attrs.insert("name".to_string(), Value::Str("c".to_string()));
    attrs.insert("amount".to_string(), Value::Str("some".to_string()));
    rt.dispatch("CountProbe::Counter.Take", attrs).expect("Take dispatches");
    assert_eq!(total(&rt), Value::Int(250), "a non-numeric decrement must move NOTHING");
}
