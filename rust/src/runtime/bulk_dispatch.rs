//! bulk_dispatch — the many-form bulk dispatch row loop (i113 / i116) :
//! detect the bulk shape (one `list_of(VO)` attribute on an `identified_by`
//! aggregate whose VO carries the identity field), iterate the spec list,
//! save each row through the standard pipeline, publish one event per row.
//! Input coercion lives in bulk_specs.rs ; the single-form path stays in
//! command_dispatch.rs.
//!
//! Cask extracted VERBATIM from runtime/command_dispatch.rs (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/bulk_dispatch.rs — kernel-floor
//!  dispatch path, relocated verbatim from command_dispatch.rs blanket.]

use super::bulk_specs::{parse_specs_from_str, spec_to_attrs};
use super::command_dispatch::CommandResult;
use super::dispatch_diagnostics::unknown_aggregate_message;
use super::lifecycle_defaults::{apply_defaults, apply_lifecycle_default};
use super::{AggregateState, Event, Runtime, RuntimeError, Value};
use std::collections::HashMap;

// ============================================================
// Many-form bulk dispatch — i113 / i116
// ============================================================
//
// A command qualifies as bulk when it takes exactly one attribute,
// that attribute is a `list_of(VO)` whose VO is declared on the same
// aggregate, the aggregate has `identified_by :field`, and the VO
// carries that identity field. The dispatcher then iterates the list,
// builds per-row attrs from the VO's fields, and runs each row through
// the standard save path so upsert + audit semantics carry through.

/// Return the bulk-spec attribute name when the command meets the bulk
/// shape contract ; otherwise None. Pure shape inspection — no naming
/// convention, no command-prefix rules.
pub(super) fn bulk_spec_attr(rt: &Runtime, agg_idx: usize, cmd_idx: usize) -> Option<String> {
    let agg = &rt.domain.aggregates[agg_idx];
    let cmd = &agg.commands[cmd_idx];
    let id_field = agg.identified_by.as_deref()?;

    if cmd.attributes.len() != 1 {
        return None;
    }
    let attr = &cmd.attributes[0];
    if !attr.list {
        return None;
    }
    let vo = agg.value_objects.iter().find(|v| v.name == attr.attr_type)?;
    let vo_has_id = vo.attributes.iter().any(|a| a.name == id_field);
    if !vo_has_id {
        return None;
    }
    Some(attr.name.clone())
}

/// Iterate the spec list, dispatch one save per row through the
/// standard pipeline, emit one event per row. The event is published
/// so policies and projections see each row exactly as if it had been
/// dispatched single-form. The returned CommandResult points at the
/// LAST row's aggregate id — callers that need every id should listen
/// on the event bus instead.
pub(super) fn dispatch_bulk(
    rt: &mut Runtime,
    agg_idx: usize,
    cmd_idx: usize,
    command_name: &str,
    spec_attr: &str,
    attrs: HashMap<String, Value>,
) -> Result<CommandResult, RuntimeError> {
    let specs = match attrs.get(spec_attr) {
        Some(Value::List(items)) => items.clone(),
        Some(Value::Str(s)) => parse_specs_from_str(s)?,
        Some(other) => {
            return Err(RuntimeError::MissingAttribute(format!(
                "{}: expected list_of(VO), got {:?}",
                spec_attr, other
            )));
        }
        None => {
            return Err(RuntimeError::MissingAttribute(spec_attr.to_string()));
        }
    };

    let aggregate_name = rt.domain.aggregates[agg_idx].name.clone();
    let event_name = rt.domain.aggregates[agg_idx].commands[cmd_idx]
        .emits
        .clone()
        .unwrap_or_else(|| format!("{}Completed", command_name));

    let vo_field_names: Vec<String> = {
        let cmd_attr = &rt.domain.aggregates[agg_idx].commands[cmd_idx].attributes[0];
        let vo = rt.domain.aggregates[agg_idx]
            .value_objects
            .iter()
            .find(|v| v.name == cmd_attr.attr_type)
            .expect("bulk_spec_attr already verified VO exists");
        vo.attributes.iter().map(|a| a.name.clone()).collect()
    };

    let mut last_id = String::new();
    let mut last_event: Option<Event> = None;
    for spec in &specs {
        let row_attrs = spec_to_attrs(spec, &vo_field_names);
        let row_id = save_one_row(rt, agg_idx, &aggregate_name, command_name, &row_attrs)?;
        let event = Event {
            name: event_name.clone(),
            aggregate_type: aggregate_name.clone(),
            aggregate_id: row_id.clone(),
            data: row_attrs,
            realm_path: rt.domain.aggregates[agg_idx].realm_path.clone(),
            ..Default::default()
        };
        // Sprint 14 (retire-sync-cascade-pipeline) — many-form
        // (`list_of(VO)`) inputs publish one event per row through the
        // bus, just like the single-event site in phase_11_emit. The
        // legacy delivery-mode fork retired with the sync-cascade
        // pipeline ; every event flows through `event_bus.publish`.
        rt.event_bus.publish(event.clone());
        last_id = row_id;
        last_event = Some(event);
    }

    Ok(CommandResult {
        aggregate_id: last_id,
        aggregate_type: aggregate_name,
        event: last_event,
        // Bulk many-form (`list_of(VO)`) event-sourcing is deferred (v1) —
        // per-row deltas would need a diff per saved row inside the loop.
        deltas: Vec::new(),
    })
}

/// Save one row through the standard pipeline — apply defaults, copy
/// matching command attrs onto the aggregate, persist, return the
/// row's id. Mirrors the create path of `dispatch` but skipped the
/// givens / lifecycle plumbing : bulk register has no per-row gates,
/// the command itself is the gate.
fn save_one_row(
    rt: &mut Runtime,
    agg_idx: usize,
    aggregate_name: &str,
    command_name: &str,
    row_attrs: &HashMap<String, Value>,
) -> Result<String, RuntimeError> {
    let repo_hash_key = super::repo_key(
        rt.domain.aggregates[agg_idx].context.as_deref(),
        aggregate_name,
    );
    // Build the diagnostic up front — borrowing `rt` inside
    // `ok_or_else` collides with the `get_mut` mutable borrow.
    let unknown_agg_msg = if rt.repositories.contains_key(&repo_hash_key) {
        String::new()
    } else {
        unknown_aggregate_message(rt, aggregate_name)
    };
    // i735 defect 2 — see the sibling guard above ; a refused-persistence
    // aggregate errors loudly here too rather than reading as unknown.
    if let Some(reason) = rt.refused_persistence.get(&repo_hash_key) {
        return Err(RuntimeError::PersistenceRefused(reason.clone()));
    }
    let repo = rt
        .repositories
        .get_mut(&repo_hash_key)
        .ok_or(RuntimeError::UnknownAggregate(unknown_agg_msg))?;
    let id = repo.id_for_command(row_attrs);
    let (mut state, is_new) = match repo.find(&id).cloned() {
        Some(s) => (s, false),
        None => (AggregateState::new(&id), true),
    };
    if is_new {
        apply_defaults(rt, agg_idx, &mut state);
        apply_lifecycle_default(rt, agg_idx, &mut state);
    }

    // Copy every VO field onto the aggregate (path, reason, …) — same
    // semantics as the single-row create path's matching-attrs copy.
    let agg_attr_names: Vec<String> = rt.domain.aggregates[agg_idx]
        .attributes
        .iter()
        .map(|a| a.name.clone())
        .collect();
    for name in &agg_attr_names {
        if let Some(val) = row_attrs.get(name) {
            state.set(name, val.clone());
        }
    }

    let ctx = crate::heki::WriteContext::Dispatch {
        aggregate: aggregate_name,
        command: command_name,
    };
    let repo = rt.repositories.get_mut(&repo_hash_key).unwrap();
    let row_id = state.id.clone();
    repo.save(state, ctx);
    Ok(row_id)
}
