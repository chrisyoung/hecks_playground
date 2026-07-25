//! loop_driver_tests — the loop-driver suite : tick counting, stop-flag
//! early exit, emit-into-bus, bootstrap predicate matching (fires /
//! silent / absent-aggregate).
//!
//! Cask extracted VERBATIM from runtime/loop_driver.rs (cask-runtime) ;
//! body dedented one level out of the old inline mod.
//!
//! [antibody-exempt: rust/src/runtime/loop_driver_tests.rs — kernel-floor
//!  loop tests, relocated verbatim from loop_driver.rs blanket.]

use super::loop_driver::*;
use super::{Runtime, Value};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::time::Duration;
use crate::parser;

fn empty_runtime() -> Runtime {
    // Minimal bluebook with one no-op aggregate — parser is the
    // canonical way to build a Domain ; constructing the IR by
    // hand bakes in field churn. `identified_by :name` so the
    // repository keys saved records by their `name` field, which
    // matches the dispatch path the i223 bootstrap predicate
    // queries against.
    let src = r#"
        Hecks.bluebook "LoopDriverTest" do
          aggregate "Tick" do
            identified_by :name
            attribute :name, :string
            attribute :state, :string
          end
        end
    "#;
    Runtime::boot(parser::parse(src))
}

#[test]
fn run_ticks_advances_tick_count() {
    let mut d = LoopDriver::new(empty_runtime(), Duration::from_millis(1));
    d.run_ticks(3);
    assert_eq!(d.tick_count(), 3);
}

#[test]
fn stop_flag_breaks_run_ticks_early() {
    let mut d = LoopDriver::new(empty_runtime(), Duration::from_millis(1));
    d.stop_flag().store(true, Ordering::Relaxed);
    d.run_ticks(10);
    assert_eq!(d.tick_count(), 0);
}

#[test]
fn emit_fires_event_into_bus() {
    let mut d = LoopDriver::new(empty_runtime(), Duration::from_millis(1));
    d.add_emit("BodyPulse", "Pulse", "pulse", HashMap::new());
    d.run_ticks(2);
    assert_eq!(d.runtime().event_bus.events().len(), 2);
    assert_eq!(d.runtime().event_bus.events()[0].name, "BodyPulse");
}

/// i223 — seed an aggregate state directly into the runtime's
/// repository. Bypasses the dispatch path (the DSL parser-built
/// runtime in this test module has no commands defined) so the
/// bootstrap predicate has something to read against.
/// Repos may key by either bare name or "Context::Name" depending
/// on whether the bluebook declared a context — find the right
/// key by suffix.
fn seed_tick_state(rt: &mut Runtime, id: &str, state_value: &str) {
    use crate::runtime::AggregateState;
    let mut s = AggregateState::new(id);
    s.set("state", Value::Str(state_value.into()));
    let key = rt.repositories.keys()
        .find(|k| k.as_str() == "Tick" || k.ends_with("::Tick"))
        .cloned()
        .expect("Tick repository must exist");
    let repo = rt.repositories.get_mut(&key).unwrap();
    let ctx = crate::heki::WriteContext::OutOfBand { reason: "test seed" };
    repo.save(s, ctx);
}

/// i223 — bootstrap predicate matches → embedded emit fires
/// once on the first tick, before regular cadence actions.
#[test]
fn bootstrap_fires_when_predicate_matches() {
    let mut rt = empty_runtime();
    seed_tick_state(&mut rt, "tick", "attentive");

    let mut d = LoopDriver::new(rt, Duration::from_millis(1));
    d.add_bootstrap(BootstrapEmit {
        aggregate_type: "Tick".into(),
        aggregate_id: "tick".into(),
        field: "state".into(),
        expected: "attentive".into(),
        event: TickAction::Emit {
            event_name: "WokenUp".into(),
            aggregate_type: "Tick".into(),
            aggregate_id: "tick".into(),
            data: HashMap::new(),
        },
    });
    d.run_ticks(2);

    let events = d.runtime().event_bus.events();
    // WokenUp from bootstrap on tick 1. No re-emit on tick 2 —
    // bootstraps drain after first tick.
    let woken = events.iter().filter(|e| e.name == "WokenUp").count();
    assert_eq!(woken, 1, "bootstrap fires exactly once");
}

/// i223 — bootstrap predicate fails → no emit, no panic.
#[test]
fn bootstrap_silent_when_predicate_does_not_match() {
    let mut rt = empty_runtime();
    seed_tick_state(&mut rt, "tick", "sleeping");

    let mut d = LoopDriver::new(rt, Duration::from_millis(1));
    d.add_bootstrap(BootstrapEmit {
        aggregate_type: "Tick".into(),
        aggregate_id: "tick".into(),
        field: "state".into(),
        expected: "attentive".into(),
        event: TickAction::Emit {
            event_name: "WokenUp".into(),
            aggregate_type: "Tick".into(),
            aggregate_id: "tick".into(),
            data: HashMap::new(),
        },
    });
    d.run_ticks(1);
    let woken = d.runtime().event_bus.events()
        .iter().filter(|e| e.name == "WokenUp").count();
    assert_eq!(woken, 0, "predicate mismatch → no emit");
}

/// i223 — bootstrap with missing aggregate → silent skip.
#[test]
fn bootstrap_silent_when_aggregate_absent() {
    let mut d = LoopDriver::new(empty_runtime(), Duration::from_millis(1));
    d.add_bootstrap(BootstrapEmit {
        aggregate_type: "Tick".into(),
        aggregate_id: "missing".into(),
        field: "state".into(),
        expected: "attentive".into(),
        event: TickAction::Emit {
            event_name: "WokenUp".into(),
            aggregate_type: "Tick".into(),
            aggregate_id: "missing".into(),
            data: HashMap::new(),
        },
    });
    d.run_ticks(1);
    let woken = d.runtime().event_bus.events()
        .iter().filter(|e| e.name == "WokenUp").count();
    assert_eq!(woken, 0);
}
