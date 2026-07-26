//! PolicyEngine — reactive event→command wiring
//!
//! Policies subscribe to events and trigger follow-up commands.
//! Reentrant execution is prevented by tracking in-flight policies.
//!
//! Usage:
//!   engine.register(&policy);   // one ir::Policy from domain.policies
//!   let triggers = engine.react(&event);

use crate::ir::{ValueSpec, WhereClause, ForEachSpec, DispatchSpec};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct PolicyBinding {
    pub name: String,
    pub on_event: String,
    /// gap #1b — bare event name (qualifier stripped). The `by_event`
    /// index keys on this so a qualified subscription still buckets
    /// under the event's name ; the qualifier is then checked against
    /// the firing event's `aggregate_type` in `react`.
    pub event_name: String,
    /// gap #1b — `Some(agg)` when the policy was declared
    /// `on "Aggregate.Event"`. The binding then fires ONLY for events
    /// of `event_name` emitted by `agg`. `None` (the bare `on "Event"`
    /// form) matches by name across every aggregate — the historical,
    /// backward-compatible behaviour.
    pub qualifier: Option<String>,
    pub trigger_command: String,
    /// Gap #1 (adapters-as-bluebook) — literal args the policy passes
    /// to its triggered command on top of the event's data. Resolved
    /// at react time and surfaced on the PolicyTrigger as `with_data`,
    /// which `drain_policies` merges into the dispatch data (winning
    /// over event data) before `inject_refs`. For this slice only
    /// `ValueSpec::Literal` is produced ; state-aware specs deferred.
    pub with: Vec<(String, ValueSpec)>,
    /// deciderate Layer 0b — data guard ; the policy fires only when EVERY
    /// clause matches the triggering event's data (evaluated in `react`).
    pub wheres: Vec<WhereClause>,
    /// deciderate Layer 0b — fan-out the primary trigger over a query ;
    /// the caller sweeps it (react has no &Runtime).
    pub for_each: Option<ForEachSpec>,
    /// deciderate Layer 0b — extra reactions beyond the primary trigger.
    pub extra_dispatches: Vec<DispatchSpec>,
}

pub struct PolicyEngine {
    pub(super) bindings: Vec<PolicyBinding>,
    pub(super) by_event: HashMap<String, Vec<usize>>,
    pub(super) in_flight: HashSet<String>,
}

#[derive(Debug)]
pub struct PolicyTrigger {
    pub policy_name: String,
    pub command_name: String,
    pub event_data: HashMap<String, super::Value>,
    /// Resolved `with` literals (gap #1). Merged over `event_data` by
    /// `drain_policies` before `inject_refs` — the policy's explicit
    /// args win over an event field of the same name.
    pub with_data: HashMap<String, super::Value>,
    /// deciderate Layer 0b — carried from the binding so the caller (which
    /// has &Runtime) can sweep the query and fan out the primary trigger.
    pub for_each: Option<ForEachSpec>,
    /// deciderate Layer 0b — extra reactions the caller fires after the
    /// primary trigger (each with its own with-spec + optional sweep).
    pub extra_dispatches: Vec<DispatchSpec>,
}

impl Default for PolicyEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl PolicyEngine {
    pub fn new() -> Self {
        PolicyEngine {
            bindings: vec![],
            by_event: HashMap::new(),
            in_flight: HashSet::new(),
        }
    }

    /// Register one policy. Takes the whole `Policy` rather than its seven fields
    /// spread as arguments — the signature mirrored the struct exactly, so this is
    /// both simpler and the same shape as `PMEngine::register(&pm)` beside it at
    /// the call site.
    pub fn register(&mut self, policy: &crate::ir::Policy) {
        let name = policy.name.as_str();
        let on_event = policy.on_event.as_str();
        let trigger_command = policy.trigger_command.as_str();
        let with = policy.with.clone();
        let wheres = policy.wheres.clone();
        let for_each = policy.for_each.clone();
        let extra_dispatches = policy.extra_dispatches.clone();
        let idx = self.bindings.len();
        // gap #1b — split `Aggregate.Event` into (qualifier, event).
        // Bare `Event` → qualifier None. The index keys on the bare
        // event name either way ; the qualifier is checked in `react`.
        let (qualifier, event_name) = match on_event.split_once('.') {
            Some((agg, ev)) => (Some(agg.to_string()), ev.to_string()),
            None => (None, on_event.to_string()),
        };
        self.bindings.push(PolicyBinding {
            name: name.to_string(),
            on_event: on_event.to_string(),
            event_name: event_name.clone(),
            qualifier,
            trigger_command: trigger_command.to_string(),
            with,
            wheres,
            for_each,
            extra_dispatches,
        });
        self.by_event
            .entry(event_name)
            .or_default()
            .push(idx);
    }

}
