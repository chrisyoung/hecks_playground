//! projection_fold_tests — the fold suite : seq-ordered replay, overlay
//! semantics, watermark handling.
//!
//! Cask extracted VERBATIM from runtime/projection_fold.rs (cask-runtime) ;
//! body dedented one level out of the old inline mod.
//!
//! [antibody-exempt: rust/src/runtime/projection_fold_tests.rs —
//!  kernel-floor fold tests, relocated verbatim from projection_fold.rs
//!  blanket.]

use super::projection_fold::*;
use super::{AggregateState, Value};
use std::collections::HashMap;

fn ev(name: &str, id: &str, seq: i64, field: &str, value: &str) -> AggregateState {
    let mut s = AggregateState::new(&format!("{}-{}-{}", name, id, seq));
    s.set("aggregate_name", Value::Str(name.to_string()));
    s.set("aggregate_id", Value::Str(id.to_string()));
    let mut delta = HashMap::new();
    delta.insert("field".to_string(), Value::Str(field.to_string()));
    delta.insert("value".to_string(), Value::Str(value.to_string()));
    s.set("delta", Value::Map(delta));
    let mut sq = HashMap::new();
    sq.insert("value".to_string(), Value::Int(seq));
    s.set("sequence", Value::Map(sq));
    s
}

#[test]
fn fold_orders_by_sequence_and_last_write_wins_per_instance() {
    // Deliberately out of order : the fold must sort by sequence.
    let e1 = ev("Sweeper", "s1", 1, "status", "idle");
    let e3 = ev("Sweeper", "s1", 3, "status", "done");
    let e2 = ev("Sweeper", "s1", 2, "status", "running");
    // A second field on the same instance, and a different instance.
    let e4 = ev("Sweeper", "s1", 2, "ticks", "7");
    let other = ev("Order", "o1", 1, "total", "5");
    let refs = vec![&e3, &e1, &e4, &other, &e2];

    let folded = fold_event_log(&refs);

    let sweeper = folded.get(&("Sweeper".to_string(), "s1".to_string())).unwrap();
    assert_eq!(sweeper.get("status").unwrap(), "done", "seq 3 must win over 1 and 2");
    assert_eq!(sweeper.get("ticks").unwrap(), "7");
    let order = folded.get(&("Order".to_string(), "o1".to_string())).unwrap();
    assert_eq!(order.get("total").unwrap(), "5");
    assert_eq!(folded.len(), 2, "two distinct instances");
}

#[test]
fn fold_forward_overlays_tail_onto_cached_rows() {
    // Cached rows at the watermark.
    let mut cached = HashMap::new();
    cached.insert("Order::o1::total".to_string(), "5".to_string());
    cached.insert("Order::o1::status".to_string(), "pending".to_string());
    // Tail (sequence > watermark) : total changes, a new instance appears.
    let e1 = ev("Order", "o1", 10, "total", "9");
    let e2 = ev("Pizza", "p1", 11, "name", "margherita");
    let out = fold_forward_onto(cached, &[&e1, &e2]);
    assert_eq!(out.get("Order::o1::total").unwrap(), "9", "tail overrides cached");
    assert_eq!(out.get("Order::o1::status").unwrap(), "pending", "untouched cached survives");
    assert_eq!(out.get("Pizza::p1::name").unwrap(), "margherita", "tail adds new instance");
}

#[test]
fn fold_forward_from_empty_is_a_full_fold() {
    // No prior snapshot (watermark 0) -> empty cached -> the tail IS the state.
    let e1 = ev("Order", "o1", 1, "total", "3");
    let e2 = ev("Order", "o1", 2, "total", "7");
    let out = fold_forward_onto(HashMap::new(), &[&e1, &e2]);
    assert_eq!(out.get("Order::o1::total").unwrap(), "7", "last write wins from empty");
    assert_eq!(out.len(), 1);
}

#[test]
fn events_without_a_delta_field_are_skipped() {
    let mut no_delta = AggregateState::new("x");
    no_delta.set("aggregate_name", Value::Str("Sweeper".into()));
    no_delta.set("aggregate_id", Value::Str("s1".into()));
    let refs = vec![&no_delta];
    assert!(fold_event_log(&refs).is_empty());
}

fn ev_at(name: &str, id: &str, seq: i64, at: &str, field: &str, value: &str) -> AggregateState {
    let mut s = ev(name, id, seq, field, value);
    s.set("recorded_at", Value::Str(at.to_string()));
    s
}

#[test]
fn recorded_at_orders_across_mixed_sequence_scales() {
    // The COMPLETE-log case : a CONSOLIDATED event carries a large AUTHORITATIVE
    // global sequence (assigned at merge) but an EARLIER wall-clock ; a fresh
    // shard TAIL event carries a small PER-PROCESS sequence but a LATER
    // wall-clock. Folding by sequence would let the stale consolidated value
    // (seq 9001) win over the fresh tail (seq 2) — the exact bug the
    // recorded_at order prevents. Last write = latest recorded_at.
    let consolidated = ev_at("InboxPoller", "p1", 9001, "2026-06-21T01:00:00Z", "last_polled_at", "2026-06-21T01:00:00Z");
    let tail = ev_at("InboxPoller", "p1", 2, "2026-06-21T01:00:02Z", "last_polled_at", "2026-06-21T01:00:02Z");
    // Order in the vec mimics consolidated-then-tail.
    let refs = vec![&consolidated, &tail];
    let folded = fold_event_log(&refs);
    let poller = folded.get(&("InboxPoller".to_string(), "p1".to_string())).unwrap();
    assert_eq!(
        poller.get("last_polled_at").unwrap(),
        "2026-06-21T01:00:02Z",
        "the later recorded_at (the tail) must win despite its smaller per-process sequence"
    );
}
