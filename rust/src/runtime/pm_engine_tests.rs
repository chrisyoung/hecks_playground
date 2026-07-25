//! pm_engine_tests — the PM engine suite : full lifecycle, correlation
//! isolation, heki persist/reload round-trip, per-instance apply_set /
//! read_attribute.
//!
//! Cask extracted VERBATIM from runtime/pm_engine.rs (cask-runtime) ;
//! body dedented one level out of the old inline mod.
//!
//! [antibody-exempt: rust/src/runtime/pm_engine_tests.rs — kernel-floor PM
//!  tests, relocated verbatim from pm_engine.rs blanket.]

use super::pm_engine::*;
use super::Event;
use crate::ir::{DispatchSpec, ProcessManager, ProcessManagerHandler};
use crate::runtime::Value;
use std::collections::HashMap;

fn order_pm() -> ProcessManager {
    ProcessManager {
        name: "OrderFulfillment".to_string(),
        correlates_by: "order_id".to_string(),
        starts_on: "OrderPlaced".to_string(),
        ends_on: Some("OrderDelivered".to_string()),
        states: vec!["pending".into(), "shipped".into(), "delivered".into()],
        handlers: vec![
            ProcessManagerHandler {
                event_type: "OrderShipped".into(),
                from_state: "pending".into(),
                to_state: "shipped".into(),
                dispatches: vec![DispatchSpec {
                    command_name: "Inventory.Decrement".into(),
                    with_spec: vec![],
                    for_each: None,
                }],
                set_specs: vec![],
            },
            ProcessManagerHandler {
                event_type: "OrderDelivered".into(),
                from_state: "shipped".into(),
                to_state: "delivered".into(),
                dispatches: vec![],
                set_specs: vec![],
            }, // dispatches: empty Vec<DispatchSpec>
        ],
    }
}

fn evt(name: &str, order_id: &str) -> Event {
    let mut data = HashMap::new();
    data.insert("order_id".into(), Value::Str(order_id.into()));
    Event {
        name: name.into(),
        aggregate_type: "Order".into(),
        aggregate_id: order_id.into(),
        data,
        realm_path: None,
        ..Default::default()
    }
}

#[test]
fn full_lifecycle() {
    let mut engine = PMEngine::new();
    engine.register(&order_pm());

    let _ = engine.react(&evt("OrderPlaced", "ord_42"));
    engine.complete("OrderFulfillment");

    let triggers = engine.react(&evt("OrderShipped", "ord_42"));
    assert_eq!(triggers.len(), 1);
    assert_eq!(triggers[0].from_state, "pending");
    assert_eq!(triggers[0].to_state, "shipped");
    assert_eq!(triggers[0].dispatches.len(), 1);
    assert_eq!(triggers[0].dispatches[0].command_name, "Inventory.Decrement");
    assert!(triggers[0].dispatches[0].with_spec.is_empty());

    engine.complete("OrderFulfillment");
    let triggers = engine.react(&evt("OrderDelivered", "ord_42"));
    assert_eq!(triggers.len(), 1);
    assert_eq!(triggers[0].to_state, "delivered");
    assert!(triggers[0].dispatches.is_empty());
}

#[test]
fn no_op_for_unrelated_event() {
    let mut engine = PMEngine::new();
    engine.register(&order_pm());
    assert_eq!(engine.react(&evt("RandomEvent", "ord_42")).len(), 0);
}

#[test]
fn isolated_correlation_ids() {
    let mut engine = PMEngine::new();
    engine.register(&order_pm());
    let _ = engine.react(&evt("OrderPlaced", "ord_1"));
    engine.complete("OrderFulfillment");
    let _ = engine.react(&evt("OrderPlaced", "ord_2"));
    engine.complete("OrderFulfillment");
    let instances = engine.instances("OrderFulfillment").unwrap();
    assert_eq!(instances.len(), 2);
}

#[test]
fn persists_and_reloads_instance_state() {
    let tmp = std::env::temp_dir().join(format!(
        "hecks_pm_persist_test_{}",
        crate::clock::now_duration()
            .as_nanos()
    ));
    let dir = tmp.to_string_lossy().to_string();
    std::fs::create_dir_all(&dir).unwrap();

    // Subprocess 1 : create instance, transition, persist.
    {
        let mut engine = PMEngine::new();
        engine.register(&order_pm());
        engine.load_persisted(Some(&dir));

        let _ = engine.react(&evt("OrderPlaced", "ord_42"));
        engine.complete("OrderFulfillment");
        engine
            .persist_instance("OrderFulfillment", "ord_42", Some(&dir))
            .unwrap();

        let _ = engine.react(&evt("OrderShipped", "ord_42"));
        engine
            .persist_instance("OrderFulfillment", "ord_42", Some(&dir))
            .unwrap();
        engine.complete("OrderFulfillment");
    }

    // Subprocess 2 : load + verify the prior state persisted.
    {
        let mut engine = PMEngine::new();
        engine.register(&order_pm());
        engine.load_persisted(Some(&dir));

        let instances = engine.instances("OrderFulfillment").unwrap();
        let inst = instances.get("ord_42").expect("instance must be loaded");
        assert_eq!(inst.state, "shipped", "state should persist across forks");
        assert_eq!(inst.last_event.as_deref(), Some("OrderShipped"));

        // And further transitions on top of loaded state work :
        let triggers = engine.react(&evt("OrderDelivered", "ord_42"));
        assert_eq!(triggers.len(), 1);
        assert_eq!(triggers[0].from_state, "shipped");
        assert_eq!(triggers[0].to_state, "delivered");
    }

    let _ = std::fs::remove_dir_all(&tmp);
}

// ---- Phase 2.c — PM attribute writes -----------------------------

#[test]
fn apply_set_writes_per_instance_attribute() {
    let mut engine = PMEngine::new();
    engine.register(&order_pm());
    let _ = engine.react(&evt("OrderPlaced", "ord_42"));
    engine.complete("OrderFulfillment");

    engine.apply_set("OrderFulfillment", "ord_42", "carrying", "body".into());
    assert_eq!(
        engine.read_attribute("OrderFulfillment", "ord_42", "carrying"),
        Some("body")
    );
}

#[test]
fn read_attribute_returns_none_when_unset() {
    let mut engine = PMEngine::new();
    engine.register(&order_pm());
    let _ = engine.react(&evt("OrderPlaced", "ord_42"));
    engine.complete("OrderFulfillment");

    assert!(engine.read_attribute("OrderFulfillment", "ord_42", "missing").is_none());
    assert!(engine.read_attribute("OrderFulfillment", "missing_id", "x").is_none());
    assert!(engine.read_attribute("Missing", "ord_42", "x").is_none());
}

#[test]
fn attributes_survive_subsequent_transitions() {
    let mut engine = PMEngine::new();
    engine.register(&order_pm());
    let _ = engine.react(&evt("OrderPlaced", "ord_42"));
    engine.complete("OrderFulfillment");
    engine.apply_set("OrderFulfillment", "ord_42", "carrying", "body".into());

    // Drive the next transition ; attribute must still be there.
    let _ = engine.react(&evt("OrderShipped", "ord_42"));
    engine.complete("OrderFulfillment");
    assert_eq!(
        engine.read_attribute("OrderFulfillment", "ord_42", "carrying"),
        Some("body")
    );
}

#[test]
fn attributes_round_trip_through_heki_persistence() {
    let tmp = std::env::temp_dir().join(format!(
        "hecks_pm_attrs_test_{}",
        crate::clock::now_duration()
            .as_nanos()
    ));
    let dir = tmp.to_string_lossy().to_string();
    std::fs::create_dir_all(&dir).unwrap();

    // Subprocess 1 : create, set attr, persist.
    {
        let mut engine = PMEngine::new();
        engine.register(&order_pm());
        engine.load_persisted(Some(&dir));
        let _ = engine.react(&evt("OrderPlaced", "ord_42"));
        engine.complete("OrderFulfillment");
        engine.apply_set("OrderFulfillment", "ord_42", "carrying", "body".into());
        engine.apply_set("OrderFulfillment", "ord_42", "tick", "7".into());
        engine
            .persist_instance("OrderFulfillment", "ord_42", Some(&dir))
            .unwrap();
    }

    // Subprocess 2 : load + verify the attributes persisted.
    {
        let mut engine = PMEngine::new();
        engine.register(&order_pm());
        engine.load_persisted(Some(&dir));
        assert_eq!(
            engine.read_attribute("OrderFulfillment", "ord_42", "carrying"),
            Some("body")
        );
        assert_eq!(
            engine.read_attribute("OrderFulfillment", "ord_42", "tick"),
            Some("7")
        );
    }

    let _ = std::fs::remove_dir_all(&tmp);
}
