//! policy_react — the PolicyEngine reaction surface : react (which
//! policies fire for an event, marks in-flight), complete /
//! reset_in_flight, bindings, trigger_commands_for (the with-spec
//! resolution feed). Registration + the binding types stay in
//! policy_engine.rs. Inherent `impl PolicyEngine` methods in a child
//! module.
//!
//! Cask extracted VERBATIM from runtime/policy_engine.rs (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/policy_react.rs — kernel-floor
//!  policy reaction, relocated verbatim from policy_engine.rs blanket.]

use super::policy_engine::{PolicyBinding, PolicyEngine, PolicyTrigger};
use super::Event;
use crate::ir::ValueSpec;
use std::collections::HashMap;

impl PolicyEngine {
    /// Check which policies should fire for this event.
    /// Returns commands to dispatch. Marks them in-flight.
    pub fn react(&mut self, event: &Event) -> Vec<PolicyTrigger> {
        let indices = match self.by_event.get(&event.name) {
            Some(v) => v.clone(),
            None => return vec![],
        };

        let mut triggers = vec![];
        for idx in indices {
            let binding = &self.bindings[idx];
            // gap #1b — an aggregate-qualified binding fires ONLY when
            // the firing event was emitted by that aggregate. A bare
            // binding (qualifier None) matches any emitter by name.
            if let Some(ref agg) = binding.qualifier {
                if agg != &event.aggregate_type {
                    continue;
                }
            }
            // deciderate Layer 0b — data guard. Build a transient state from
            // the event data ; require every clause to match (reuses the
            // canonical where_matches — no duplicate matcher).
            if !binding.wheres.is_empty() {
                let mut guard_state = super::AggregateState::new("");
                guard_state.fields = event.data.clone();
                let no_attrs: HashMap<String, String> = HashMap::new();
                if !binding
                    .wheres
                    .iter()
                    .all(|w| super::where_matches(&guard_state, w, &no_attrs))
                {
                    continue;
                }
            }
            if self.in_flight.contains(&binding.name) {
                continue;
            }
            self.in_flight.insert(binding.name.clone());
            // Resolve the `with` literals (gap #1). This slice only
            // produces `ValueSpec::Literal`, so resolution is direct ;
            // state-aware specs (FromState/templating) will route
            // through the runtime's evaluate_value_spec when added for
            // later families.
            let mut with_data: HashMap<String, super::Value> = HashMap::new();
            for (key, spec) in &binding.with {
                match spec {
                    ValueSpec::Literal { value } => {
                        with_data.insert(key.clone(), super::Value::Str(value.clone()));
                    }
                    // Cross-field rename (`map account: :destination`) : pull
                    // the named field off the TRIGGERING event, falling back to
                    // a declared default. This is what routes a cascade — the
                    // triggered command learns which aggregate to act on and
                    // carries the event's amount / provenance forward. Without
                    // it the rename is silently dropped and the cascade
                    // mis-routes to the upstream (the transfer-saga root bug).
                    ValueSpec::FromEvent { name, default } => {
                        if let Some(v) = event.data.get(name) {
                            with_data.insert(key.clone(), v.clone());
                        } else if name == "id" {
                            // Provenance : `:id` names the EMITTING aggregate's
                            // own identity, which rides the Event as aggregate_id
                            // (not a data field). Lets a cascade carry the source
                            // aggregate forward — a transfer's Deposit echoes the
                            // Transfer id so the completion policy resolves it.
                            // Resolved narrowly here (never injected into
                            // event.data), so it can't shadow a triggered
                            // command's own self-ref id fallback.
                            with_data.insert(key.clone(), super::Value::Str(event.aggregate_id.clone()));
                        } else if let Some(d) = default {
                            with_data.insert(key.clone(), super::Value::Str(d.clone()));
                        }
                    }
                    _ => {}
                }
            }
            triggers.push(PolicyTrigger {
                policy_name: binding.name.clone(),
                command_name: binding.trigger_command.clone(),
                event_data: event.data.clone(),
                with_data,
                for_each: binding.for_each.clone(),
                extra_dispatches: binding.extra_dispatches.clone(),
            });
        }
        triggers
    }

    /// Clear in-flight marker after policy command completes
    pub fn complete(&mut self, policy_name: &str) {
        self.in_flight.remove(policy_name);
    }

    /// C3 cutover — clear ALL in-flight markers. Called after an outbox pump
    /// session drains to quiescence so the NEXT session starts with a fresh
    /// reentrancy guard. The guard breaks cyclic cascades WITHIN a session ;
    /// across sessions it must reset, or a long-running LoopDriver (one
    /// Runtime across many ticks) would skip every policy after tick 1.
    pub fn reset_in_flight(&mut self) {
        self.in_flight.clear();
    }

    pub fn bindings(&self) -> &[PolicyBinding] {
        &self.bindings
    }

    /// C1 (transactional outbox) — enumerate, WITHOUT firing, the policy
    /// trigger commands that WOULD react to an event. Read-only mirror of
    /// `react`'s selection logic (the qualifier match), minus the
    /// `in_flight` mutation. Used to PLAN a CascadeRun's steps for the
    /// persistent outbox before any reaction executes.
    pub fn trigger_commands_for(
        &self,
        event_name: &str,
        aggregate_type: &str,
    ) -> Vec<(String, Vec<(String, String)>)> {
        let indices = match self.by_event.get(event_name) {
            Some(v) => v,
            None => return vec![],
        };
        indices
            .iter()
            .filter_map(|&idx| {
                let b = &self.bindings[idx];
                if let Some(ref agg) = b.qualifier {
                    if agg != aggregate_type {
                        return None;
                    }
                }
                // `with` literals (static Literal specs only, mirroring react).
                let withs: Vec<(String, String)> = b
                    .with
                    .iter()
                    .filter_map(|(k, spec)| match spec {
                        ValueSpec::Literal { value } => Some((k.clone(), value.clone())),
                        _ => None,
                    })
                    .collect();
                Some((b.trigger_command.clone(), withs))
            })
            .collect()
    }
}
