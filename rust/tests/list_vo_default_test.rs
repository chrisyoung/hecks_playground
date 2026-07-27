//! list_vo_default_test.rs — regression for the apply_defaults branch-order
//! bug found via the miette proprioception/being suites (2026-07-27) : a
//! `list_of(X)` attribute whose element VO declares ANY member default was
//! initialised with the VO's canonical Map (the rich-VO branch ran before
//! the list branch), so `then_set … append:` piled onto a non-list and the
//! behaviors `_size` accessor read null. A declared list must start [].
//!
//! [antibody-exempt: rust/tests/list_vo_default_test.rs — kernel-surface
//!  regression test asserting Rust runtime state ; same category as
//!  validator_rules_test.rs.]

use std::collections::HashMap;
use storehouse::parser;
use storehouse::runtime::{Runtime, Value};

const BLUEBOOK: &str = r#"Hecks.bluebook "AppendProbe" do
  aggregate "Bag" do
    identified_by :name
    attribute :name, BagName
    attribute :items, list_of(Item)
    value_object "BagName" do
      attribute :value, String
    end
    value_object "Item" do
      attribute :label, String
      attribute :flag, default: true
    end
    command "MakeBag" do
      role "System"
      attribute :name, BagName
      then_set :name, to: :name
      emits "BagMade"
    end
    command "AddItem" do
      role "System"
      attribute :label, String
      then_set :items, append: { label: :label }
      emits "ItemAdded"
    end
  end
end"#;

#[test]
fn list_of_with_rich_vo_element_starts_empty_and_appends() {
    let domain = parser::parse(BLUEBOOK);
    let mut rt = Runtime::boot_with_data_dir(domain, None);

    let mut attrs = HashMap::new();
    attrs.insert("name".to_string(), Value::Str("sample".to_string()));
    rt.dispatch("AppendProbe::Bag.MakeBag", attrs).expect("MakeBag dispatches");

    let bags = rt.all("Bag");
    let items = bags[0].get("items");
    assert!(
        matches!(items, Value::List(l) if l.is_empty()),
        "a declared list must start [] even when its element VO has member \
         defaults (got {items:?})"
    );

    let mut attrs = HashMap::new();
    attrs.insert("label".to_string(), Value::Str("thing".to_string()));
    attrs.insert("name".to_string(), Value::Str("sample".to_string()));
    rt.dispatch("AppendProbe::Bag.AddItem", attrs).expect("AddItem dispatches");

    let bags = rt.all("Bag");
    let items = bags[0].get("items");
    assert!(
        matches!(items, Value::List(l) if l.len() == 1),
        "append must land exactly one element (got {items:?})"
    );
}
