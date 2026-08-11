//! PMEngine — process_manager runtime execution
//!
//! [antibody-exempt: rust/src/runtime/pm_engine.rs — generic state-machine
//!  interpreter for process_manager IR. PMs are bluebook ; this Rust
//!  runtime that drives them is kernel floor at L_a (mirror of
//!  policy_engine.rs). The deeper L_b lift — `runtime_engine` primitive
//!  unifying PolicyEngine + PMEngine + WorkflowExecutor as one
//!  declarable kind — is filed as a follow-on branch (see
//!  miette/dream-study/DEBRIEF.md → "Bluebook-First backlog").]
//!
//! Stateful coordinator for `process_manager` declarations. Each
//! `ProcessManager` IR registers its handlers ; on each event published
//! to the bus, PMEngine looks up bindings, finds matching PM instances
//! by correlation_id, transitions state per from/to map, and returns
//! commands to dispatch via the handler's declared `dispatch` list.
//!
//! Mirrors `PolicyEngine` shape — both react to events ; PMEngine adds
//! per-correlation state.
//!
//! Two layers of bluebook-first :
//!   - L_a (this file) : PMs are bluebook ; this generic interpreter
//!     drives them. Kernel floor at this layer.
//!   - L_b (follow-on `runtime_engine` branch) : the engine itself
//!     declared as bluebook ; one interpreter walks all engine kinds
//!     (Policy + PM + Workflow). Future work.

use super::Event;
use crate::ir::{DispatchSpec, ProcessManager, ProcessManagerHandler};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct PMBinding {
    pub name: String,
    pub correlates_by: String,
    pub starts_on: String,
    pub ends_on: Option<String>,
    pub states: Vec<String>,
    pub handlers: Vec<ProcessManagerHandler>,
    pub instances: HashMap<String, PMInstanceState>,
}

#[derive(Debug, Clone)]
pub struct PMInstanceState {
    pub correlation_id: String,
    pub state: String,
    pub last_event: Option<String>,
    /// Per-instance attribute storage. Phase 2.c
    /// (pm-attribute-writes) — writes flow in from `set :attr, ...`
    /// directives in PM handlers ; reads flow out via `from_pm(:attr)`
    /// in the same handlers' dispatch with-specs. Stringly typed to
    /// match the existing ValueSpec literal/default convention ; the
    /// dispatch evaluator wraps reads as `Value::Str`.
    pub attributes: HashMap<String, String>,
}

/// What PMEngine returns when an event triggers state changes.
/// `dispatches` carries the structured DispatchSpec entries declared
/// via the handler's `dispatch "Cmd", with: { ... }` lines ; caller
/// evaluates each spec's with_spec at dispatch time and routes
/// through the cascade dispatcher.
#[derive(Debug, Clone)]
pub struct PMTrigger {
    pub pm_name: String,
    pub correlation_id: String,
    pub from_state: String,
    pub to_state: String,
    pub event_name: String,
    pub dispatches: Vec<DispatchSpec>,
}

pub struct PMEngine {
    pub(super) bindings: HashMap<String, PMBinding>,
    in_flight: HashSet<String>,
}

impl PMEngine {
    pub fn new() -> Self {
        PMEngine {
            bindings: HashMap::new(),
            in_flight: HashSet::new(),
        }
    }

    pub fn register(&mut self, pm_ir: &ProcessManager) {
        let existing_instances = self
            .bindings
            .get(&pm_ir.name)
            .map(|b| b.instances.clone())
            .unwrap_or_default();
        self.bindings.insert(
            pm_ir.name.clone(),
            PMBinding {
                name: pm_ir.name.clone(),
                correlates_by: pm_ir.correlates_by.clone(),
                starts_on: pm_ir.starts_on.clone(),
                ends_on: pm_ir.ends_on.clone(),
                states: pm_ir.states.clone(),
                handlers: pm_ir.handlers.clone(),
                instances: existing_instances,
            },
        );
    }

    pub fn react(&mut self, event: &Event) -> Vec<PMTrigger> {
        let mut triggers = vec![];
        let names: Vec<String> = self.bindings.keys().cloned().collect();
        for pm_name in names {
            if self.in_flight.contains(&pm_name) {
                continue;
            }
            if let Some(t) = self.try_react_one(&pm_name, event) {
                self.in_flight.insert(pm_name.clone());
                triggers.push(t);
            }
        }
        triggers
    }

    fn try_react_one(&mut self, pm_name: &str, event: &Event) -> Option<PMTrigger> {
        let binding = self.bindings.get_mut(pm_name)?;
        let correlation_id = extract_correlation_id(event, &binding.correlates_by)?;

        // Get-or-create-on-starts_on : starts_on creates fresh instance
        // in first declared state ; other events for non-existent
        // instances are ignored.
        let current_state = match binding.instances.get(&correlation_id) {
            Some(inst) => inst.state.clone(),
            None => {
                if event.name == binding.starts_on {
                    let initial = binding.states.first().cloned().unwrap_or_default();
                    binding.instances.insert(
                        correlation_id.clone(),
                        PMInstanceState {
                            correlation_id: correlation_id.clone(),
                            state: initial.clone(),
                            last_event: Some(event.name.clone()),
                            attributes: HashMap::new(),
                        },
                    );
                    initial
                } else {
                    return None;
                }
            }
        };

        let handler = binding
            .handlers
            .iter()
            .find(|h| h.event_type == event.name && h.from_state == current_state)?;
        let from_state = handler.from_state.clone();
        let to_state = handler.to_state.clone();
        let dispatches = handler.dispatches.clone();

        // Preserve per-instance attributes across the transition. Set
        // directives evaluated by the runtime later (drain_pms) mutate
        // them in place via `apply_set` ; this clone keeps anything
        // that was set on a previous handler firing intact when the
        // current handler doesn't write to that key.
        let prior_attributes = binding
            .instances
            .get(&correlation_id)
            .map(|inst| inst.attributes.clone())
            .unwrap_or_default();

        binding.instances.insert(
            correlation_id.clone(),
            PMInstanceState {
                correlation_id: correlation_id.clone(),
                state: to_state.clone(),
                last_event: Some(event.name.clone()),
                attributes: prior_attributes,
            },
        );

        Some(PMTrigger {
            pm_name: pm_name.to_string(),
            correlation_id,
            from_state,
            to_state,
            event_name: event.name.clone(),
            dispatches,
        })
    }

    pub fn complete(&mut self, pm_name: &str) {
        self.in_flight.remove(pm_name);
    }

    /// Phase 2.c — apply one resolved (attr, value) pair to the named
    /// PM instance's per-instance attribute map. Caller (Runtime
    /// drain_pms) evaluates the handler's `set_specs` ValueSpecs first
    /// (using the same evaluator as `with_spec`), then walks the
    /// resolved pairs through `apply_set`. No-op when the binding or
    /// instance is missing — drain_pms only reaches this after a
    /// successful PMTrigger so the row exists in normal flow.
    pub fn apply_set(&mut self, pm_name: &str, correlation_id: &str, attr: &str, value: String) {
        if let Some(binding) = self.bindings.get_mut(pm_name) {
            if let Some(inst) = binding.instances.get_mut(correlation_id) {
                inst.attributes.insert(attr.to_string(), value);
            }
        }
    }

    /// Phase 2.c — read one attribute off a PM instance for
    /// from_pm(:attr) evaluation. Returns None when the binding,
    /// instance, or attribute is absent ; the caller falls back to
    /// the ValueSpec's `default`.
    pub fn read_attribute(&self, pm_name: &str, correlation_id: &str, attr: &str) -> Option<&str> {
        self.bindings
            .get(pm_name)
            .and_then(|b| b.instances.get(correlation_id))
            .and_then(|inst| inst.attributes.get(attr).map(|s| s.as_str()))
    }

    pub fn bindings(&self) -> impl Iterator<Item = &PMBinding> {
        self.bindings.values()
    }

    pub fn instances(&self, pm_name: &str) -> Option<&HashMap<String, PMInstanceState>> {
        self.bindings.get(pm_name).map(|b| &b.instances)
    }

}
fn extract_correlation_id(event: &Event, correlates_by: &str) -> Option<String> {
    if let Some(v) = event.data.get(correlates_by) {
        return Some(v.to_string());
    }
    Some(event.aggregate_id.clone())
}

impl Default for PMEngine {
    fn default() -> Self {
        Self::new()
    }
}
