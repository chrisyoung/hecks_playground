//! lifecycle_defaults — the lifecycle + defaults half of command dispatch :
//! resolution-aware self-ref lookup (i111-J), lifecycle gate + transition,
//! default seeding on fresh aggregates (attribute defaults, lifecycle
//! default state, VO canonical instances), and event construction with the
//! default event-name rule. The dispatch loop that calls these lives in
//! command_dispatch.rs.
//!
//! Cask extracted VERBATIM from runtime/command_dispatch.rs (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/lifecycle_defaults.rs — kernel-floor
//!  dispatch path, relocated verbatim from command_dispatch.rs blanket.]

use super::command_dispatch::{cmd_for, lifecycle_for, Resolution};
use super::{AggregateState, Event, Runtime, RuntimeError, Value};
use std::collections::HashMap;

/// Resolution-aware self-ref finder (i111-J). For an entity command,
/// the self-ref still points at the parent aggregate's record because
/// the entity is reached through the root. The reference's actual
/// name (which may be aliased via `role: :incident_id`) is what the
/// runner / DSL passes the kwarg under ; the runtime must look up
/// under the same key.
pub(super) fn find_self_ref_res(rt: &Runtime, res: Resolution) -> Option<String> {
    let agg_idx = res.agg_idx();
    let agg = &rt.domain.aggregates[agg_idx];
    let cmd = cmd_for(rt, res);
    let agg_snake = to_snake_case(&agg.name);
    for r in &cmd.references {
        let ref_snake = to_snake_case(&r.target);
        if ref_snake == agg_snake || agg_snake.ends_with(&ref_snake) {
            return Some(r.name.clone());
        }
    }
    None
}

/// Verify the command is allowed given the current lifecycle state.
/// Routes through Resolution so entity commands consult the entity's
/// lifecycle when present (falls back to the parent aggregate's
/// lifecycle otherwise).
pub(super) fn check_lifecycle(
    rt: &Runtime, res: Resolution, state: &AggregateState,
) -> Result<(), RuntimeError> {
    let lifecycle = match lifecycle_for(rt, res) {
        Some(lc) => lc,
        None => return Ok(()),
    };
    let cmd = cmd_for(rt, res);
    let matching: Vec<_> = lifecycle.transitions.iter()
        .filter(|t| t.command == cmd.name).collect();
    if matching.is_empty() { return Ok(()); }

    let current = format!("{}", state.get(&lifecycle.field));
    let allowed = matching.iter().any(|t| match &t.from_state {
        Some(from) => current == *from,
        None => true,
    });
    if allowed { Ok(()) } else {
        Err(RuntimeError::LifecycleViolation {
            command: cmd.name.clone(),
            field: lifecycle.field.clone(),
            current,
            allowed: matching.iter().filter_map(|t| t.from_state.clone()).collect(),
        })
    }
}

/// Apply the lifecycle transition if one matches the resolved command.
/// Resolution-aware (i111-J) — entity commands consult the entity's
/// lifecycle when present, otherwise the parent aggregate's.
pub(super) fn apply_lifecycle_transition(rt: &Runtime, res: Resolution, state: &mut AggregateState) {
    let lifecycle = match lifecycle_for(rt, res) {
        Some(lc) => lc,
        None => return,
    };
    let cmd = cmd_for(rt, res);
    let current = format!("{}", state.get(&lifecycle.field));
    for t in &lifecycle.transitions {
        if t.command != cmd.name { continue; }
        let from_ok = match &t.from_state {
            Some(from) => current == *from,
            None => true,
        };
        if from_ok {
            state.set(&lifecycle.field, Value::Str(t.to_state.clone()));
            return;
        }
    }
}

pub(crate) fn apply_lifecycle_default(rt: &Runtime, agg_idx: usize, state: &mut AggregateState) {
    let agg = &rt.domain.aggregates[agg_idx];
    if let Some(ref lc) = agg.lifecycle {
        if matches!(state.get(&lc.field), Value::Null) {
            state.set(&lc.field, Value::Str(lc.default.clone()));
        }
    }
}

pub(super) fn apply_defaults(rt: &Runtime, agg_idx: usize, state: &mut AggregateState) {
    let agg = &rt.domain.aggregates[agg_idx];
    for attr in &agg.attributes {
        if let Some(ref default) = attr.default {
            state.set(&attr.name, parse_default(default, &attr.attr_type));
        } else if attr.list {
            // A declared list ALWAYS starts empty — and this branch runs
            // BEFORE the rich-VO branch, because a VO's canonical default is
            // the zero of ONE element, never the list's zero. When the VO
            // branch ran first, `list_of(Limb)` (Limb carrying any member
            // default) initialised as the VO's Map — append piled onto a
            // non-list and `_size` read null. Surfaced by the proprioception +
            // being suites (fixtures→policies arc, 2026-07-27).
            //
            // ONLY a declared list gets the placeholder. This used to read
            // `attr.list || vo_names.contains(&attr.attr_type)` — the
            // AUTO-LIST HEURISTIC, which the locked convention retired from
            // both parsers (`attribute :foos, Foo` is SCALAR ; lists require
            // `list_of(X)`).
            state.set(&attr.name, Value::List(vec![]));
        } else if let Some(vo_default) = construct_vo_default(agg, &attr.attr_type) {
            // A value object with inner-attribute defaults self-constructs
            // its canonical instance (Money{cents:0, currency:{code:"USD"}}) :
            // a RICH value object owns its zero, no hand-written then_set.
            // Opt-in — only a VO that declares inner defaults materialises ;
            // a defaultless VO stays unset, so this never injects an empty map.
            state.set(&attr.name, vo_default);
        }
    }
}

/// Resolution-aware event builder (i111-J). Entity commands emit
/// events tagged with the parent aggregate's name — that's the
/// addressable record from outside the bounded context — but use
/// the entity command's `emits:` declaration when present.
pub(super) fn build_event_res(
    rt: &Runtime, res: Resolution,
    aggregate_id: &str, attrs: &HashMap<String, Value>,
) -> Option<Event> {
    let agg_idx = res.agg_idx();
    let agg = &rt.domain.aggregates[agg_idx];
    let cmd = cmd_for(rt, res);
    let event_name = cmd.emits.clone().unwrap_or_else(|| default_event_name(&cmd.name));
    Some(Event {
        name: event_name,
        aggregate_type: agg.name.clone(),
        aggregate_id: aggregate_id.to_string(),
        data: attrs.clone(),
        realm_path: agg.realm_path.clone(),
        // Lineage stamped downstream : event_id by publish, causation_id by the
        // dispatcher from the cascade hint (causation-tree view).
        ..Default::default()
    })
}

fn default_event_name(cmd_name: &str) -> String {
    if let Some(rest) = cmd_name.strip_prefix("Create") {
        format!("{}Created", rest)
    } else if let Some(rest) = cmd_name.strip_prefix("Add") {
        format!("{}Added", rest)
    } else if let Some(rest) = cmd_name.strip_prefix("Place") {
        format!("{}Placed", rest)
    } else if let Some(rest) = cmd_name.strip_prefix("Cancel") {
        format!("{}Cancelled", rest)
    } else if let Some(rest) = cmd_name.strip_prefix("Update") {
        format!("{}Updated", rest)
    } else if let Some(rest) = cmd_name.strip_prefix("Remove") {
        format!("{}Removed", rest)
    } else if let Some(rest) = cmd_name.strip_prefix("Delete") {
        format!("{}Deleted", rest)
    } else {
        format!("{}Completed", cmd_name)
    }
}

/// Build a value object's CANONICAL instance from its inner-attribute defaults,
/// recursively : a nested-VO field (e.g. Money's `currency: Currency`) recurses
/// into its own inner defaults. Returns `None` when `vo_name` is not a value
/// object of this aggregate, OR the VO declares no inner defaults — so
/// materialisation is OPT-IN (declaring an inner `default:`) and a defaultless
/// VO never becomes an empty map. This makes the inner-attribute `default:`
/// form (parsed + dumped since day one) finally MATERIALISE at construction.
fn construct_vo_default(agg: &crate::ir::Aggregate, vo_name: &str) -> Option<Value> {
    let vo = agg.value_objects.iter().find(|v| v.name == vo_name)?;
    let mut map = std::collections::HashMap::new();
    for attr in &vo.attributes {
        if let Some(ref d) = attr.default {
            map.insert(attr.name.clone(), parse_default(d, &attr.attr_type));
        } else if let Some(nested) = construct_vo_default(agg, &attr.attr_type) {
            map.insert(attr.name.clone(), nested);
        }
    }
    if map.is_empty() { None } else { Some(Value::Map(map)) }
}

pub(super) fn parse_default(default: &str, attr_type: &str) -> Value {
    match attr_type {
        "Integer" => Value::Int(default.parse().unwrap_or(0)),
        "Boolean" => Value::Bool(default == "true"),
        _ => Value::Str(default.trim_matches('"').to_string()),
    }
}

fn to_snake_case(s: &str) -> String {
    crate::parser_helpers::to_snake_case(s)
}
