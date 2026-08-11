//! driven_adapter_tests — the driving-side resolver suite : quote-aware
//! split_argv, .world-vs-canned pick, declared-wins merge, realm/context
//! enforcement in event_ref_matches.
//!
//! Cask extracted VERBATIM from runtime/driven_adapter_resolver.rs
//! (cask-runtime) ; body dedented one level out of the old inline mod.
//!
//! [antibody-exempt: rust/src/runtime/driven_adapter_tests.rs —
//!  kernel-floor driving-side tests, relocated verbatim from
//!  driven_adapter_resolver.rs blanket.]

use super::driven_adapter_args::*;
use super::Value;
use crate::hecksagon_ir::CannedResponse;

fn canned(pairs: &[(&str, &str)]) -> CannedResponse {
    CannedResponse {
        values: pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
    }
}

fn raw_pairs(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

// Sprint 14 memory-canned-defaults : adapter with `canned do` and no
// `.world` entry → the canned values ARE the wrapped-call return.
#[test]
fn split_argv_keeps_quoted_jq_filter_as_one_arg() {
    // A --jq filter is single-quoted and contains double-quotes + spaces ;
    // it must arrive as ONE argv token with the inner double-quotes intact.
    let cmd = "gh pr view sq/x --jq 'if .a==\"OPEN\" then empty else halt_error(1) end'";
    let parts = split_argv(cmd);
    assert_eq!(parts[0], "gh");
    assert_eq!(parts[3], "sq/x");
    assert_eq!(parts[4], "--jq");
    assert_eq!(parts[5], "if .a==\"OPEN\" then empty else halt_error(1) end");
    assert_eq!(parts.len(), 6, "quoted filter must not be split on its inner spaces");
}

#[test]
fn pick_returns_canned_when_no_world_binding() {
    let c = canned(&[("output", "\"canned-ack\""), ("exit_code", "0")]);
    let picked = pick_wrapped_values(None, Some(&c));
    assert_eq!(
        picked,
        raw_pairs(&[("output", "\"canned-ack\""), ("exit_code", "0")]),
    );
}

// Sprint 14 world-wires-real-adapters : adapter with `canned do` AND
// a `.world` entry → the .world binding's values ARE the wrapped-call
// return ; canned is bypassed.
#[test]
fn pick_returns_world_when_binding_present_canned_ignored() {
    let c = canned(&[("output", "\"canned-ack\"")]);
    let world = raw_pairs(&[("output", "\"real-ack\"")]);
    let picked = pick_wrapped_values(Some(&world), Some(&c));
    assert_eq!(picked, raw_pairs(&[("output", "\"real-ack\"")]));
}

// Sprint 14 legacy : adapter with neither canned nor world → empty
// wrapped-call return ; only the declared dispatch attrs reach the
// follow-on.
#[test]
fn pick_returns_empty_when_neither_present() {
    let picked = pick_wrapped_values(None, None);
    assert!(picked.is_empty());
}

// Sprint 14 — declared dispatch attrs override on key collision so
// `dispatch "Y", id: "fixed"` pins `id` regardless of the canned or
// world source.
#[test]
fn merge_declared_wins_over_wrapped_on_key_collision() {
    let wrapped = raw_pairs(&[("id", "\"canned-id\""), ("output", "\"ack\"")]);
    let declared = raw_pairs(&[("id", "\"fixed\"")]);
    let merged = merge_wrapped_and_declared(&wrapped, &declared);
    // declared wins for `id` ...
    assert_eq!(merged.get("id"), Some(&Value::Str("fixed".to_string())));
    // ... and wrapped survives for keys declared doesn't shadow.
    assert_eq!(merged.get("output"), Some(&Value::Str("ack".to_string())));
}

// Sprint 14 — wrapped value reaches the follow-on when declared
// doesn't shadow the key, so the canned/world source actually wires
// through to the dispatch.
#[test]
fn merge_wrapped_keys_reach_follow_on_when_declared_silent() {
    let wrapped = raw_pairs(&[("output", "\"real-ack\""), ("exit_code", "0")]);
    let declared = raw_pairs(&[]);
    let merged = merge_wrapped_and_declared(&wrapped, &declared);
    assert_eq!(merged.get("output"), Some(&Value::Str("real-ack".to_string())));
    assert_eq!(merged.get("exit_code"), Some(&Value::Int(0)));
}

// Event routing is realm-aware (symmetric with the command resolver) :
// a `driven on` ref must match the EMITTER's stamped realm_path, carried
// on the event. Legacy 2-seg refs stay lenient ; cross-realm no longer fires.
#[test]
fn event_ref_matches_enforces_realm_and_context() {
    use crate::runtime::event_bus::Event;
    use std::collections::HashMap;
    let evt = Event {
        name: "Polled".into(),
        aggregate_type: "InboxPoller".into(),
        aggregate_id: "x".into(),
        data: HashMap::new(),
            realm_path: Some("hecks/framework".into()),
            ..Default::default()
        };
    // Canonical ref matching the emitter's realm + context → fires.
    assert!(event_ref_matches("Hecks::Framework::AgentInbox::InboxPoller.Polled", &evt));
    // Legacy 2-seg ref → lenient, still fires.
    assert!(event_ref_matches("InboxPoller.Polled", &evt));
    // Wrong realm → rejected.
    assert!(!event_ref_matches("Miette::Body::AgentInbox::InboxPoller.Polled", &evt));
    // Wrong context → rejected.
    assert!(!event_ref_matches("Hecks::Wrongctx::AgentInbox::InboxPoller.Polled", &evt));
    // Wrong event name → rejected.
    assert!(!event_ref_matches("Hecks::Framework::AgentInbox::InboxPoller.Other", &evt));
}
