//! driven_adapter_enumerate — C3 (transactional outbox) : enumerate,
//! WITHOUT dispatching, the driven-adapter follow-on commands an event
//! would fire — the read-only mirror of the resolver's snapshot phase
//! (handler match + for_each sweep + wrapped/declared merge), returning
//! (command, attrs) for the outbox to record as Steps. The live resolver
//! stays in driven_adapter_resolver.rs.
//!
//! Cask extracted VERBATIM from runtime/driven_adapter_resolver.rs
//! (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/driven_adapter_enumerate.rs —
//!  kernel-floor outbox capture, relocated verbatim from
//!  driven_adapter_resolver.rs blanket.]

use super::driven_adapter_args::*;
use crate::runtime::event_bus::Event;
use crate::runtime::{Runtime, Value};

/// C3 (transactional outbox) — enumerate, WITHOUT dispatching, the driven-
/// adapter follow-on COMMANDS that would fire for an event. Read-only mirror
/// of the snapshot phase of `resolve_driven_adapters_at_depth` (handler match
/// + for_each sweep + wrapped/declared attr merge), returning (command,
/// attrs) so the outbox can record them as Steps the pump delivers later.
/// Only `dispatches` are captured — the impure `runs`/`checks` adapter
/// executions stay on the eager edge.
pub fn enumerate_driven_dispatches(
    rt: &Runtime,
    event: &Event,
) -> Vec<(String, std::collections::HashMap<String, Value>)> {
    let mut out: Vec<(String, std::collections::HashMap<String, Value>)> = Vec::new();
    if rt.hecksagons.is_empty() {
        return out;
    }
    for hex in rt.hecksagons.iter() {
        for adapter in hex.driven_adapters.iter() {
            let world_values: Option<Vec<(String, String)>> = rt
                .world_adapter_bindings
                .iter()
                .find(|b| b.name == adapter.name)
                .map(|b| b.values.clone());
            for handler in adapter.handlers.iter() {
                if !event_ref_matches(&handler.event_ref, event) {
                    continue;
                }
                let wrapped =
                    pick_wrapped_values(world_values.as_deref(), handler.canned.as_ref());
                for dispatch in handler.dispatches.iter() {
                    match &dispatch.for_each {
                        None => {
                            let interp_attrs: Vec<(String, String)> = dispatch
                                .attrs
                                .iter()
                                .map(|(k, v)| (k.clone(), interpolate_event(v, event)))
                                .collect();
                            out.push((
                                dispatch.command.clone(),
                                merge_wrapped_and_declared(&wrapped, &interp_attrs),
                            ));
                        }
                        Some(spec) => {
                            let qattrs = sweep_query_attrs(&spec.query_inputs, event);
                            for record in rt.sweep_records(spec, &qattrs) {
                                let interp_attrs: Vec<(String, String)> = dispatch
                                    .attrs
                                    .iter()
                                    .map(|(k, v)| {
                                        (k.clone(), interpolate_event_and_record(v, event, &record))
                                    })
                                    .collect();
                                out.push((
                                    dispatch.command.clone(),
                                    merge_wrapped_and_declared(&wrapped, &interp_attrs),
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
    out
}
