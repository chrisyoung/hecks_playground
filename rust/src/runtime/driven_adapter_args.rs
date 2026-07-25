//! driven_adapter_args — the argument/interpolation layer of the driven
//! adapter resolver : val_str, split_argv (quote-aware), interpolate_event
//! (+ _and_record), event_ref_matches (realm/context enforcement),
//! build_attr_map, pick_wrapped_values / merge_wrapped_and_declared (the
//! .world-vs-canned pick + declared-wins merge), sweep_query_attrs. The
//! resolver loop stays in driven_adapter_resolver.rs.
//!
//! Cask extracted VERBATIM from runtime/driven_adapter_resolver.rs
//! (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/driven_adapter_args.rs — kernel-floor
//!  driving-side args, relocated verbatim from driven_adapter_resolver.rs
//!  blanket.]

use super::{Event, Value};
use std::collections::HashMap;

pub(super) fn val_str(v: &Value) -> String {
    match v {
        Value::Str(s) => s.clone(),
        Value::Int(i) => i.to_string(),
        Value::Bool(b) => b.to_string(),
        _ => String::new(),
    }
}

/// Split a command line into argv, honoring single and double quotes so one
/// argument may contain spaces (e.g. a `--jq` filter). A quoted span groups
/// into the current token and its content is taken literally ; the alternate
/// quote char inside passes through verbatim (so `'if .x=="Y"'` is one arg
/// with the inner double-quotes preserved). No escape processing — sufficient
/// for the git / gh+jq probes the live-check leaves run.
pub(super) fn split_argv(cmd: &str) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut has = false;
    let mut chars = cmd.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            ' ' | '\t' | '\n' | '\r' => {
                if has { args.push(std::mem::take(&mut cur)); has = false; }
            }
            '\'' | '"' => {
                has = true;
                let q = c;
                for d in chars.by_ref() {
                    if d == q { break; }
                    cur.push(d);
                }
            }
            _ => { has = true; cur.push(c); }
        }
    }
    if has { args.push(cur); }
    args
}

/// Interpolate `{field}` tokens in a run-command template from the
/// triggering event's data + identity. Identity/event fields only —
/// there are no FK refs here ; `{worktree_path}` is the aggregate's own id.
pub(super) fn interpolate_event(template: &str, event: &Event) -> String {
    let mut out = template.to_string();
    for (k, v) in event.data.iter() {
        out = out.replace(&format!("{{{}}}", k), &val_str(v));
    }
    out = out.replace("{id}", &event.aggregate_id);
    out = out.replace("{aggregate_id}", &event.aggregate_id);
    // The `{now}` / `{now+N}` / `{now-N}` clock primitive. Resolved LAST
    // so a field literally named `now` (event data) still wins ; only the
    // unresolved clock tokens reach the time resolver. This single call is
    // why the clock reaches every driven write path : the per-record sweep
    // attrs (interpolate_event_and_record) and the query `where` values
    // (sweep_query_attrs) both funnel through interpolate_event.
    out = crate::runtime::storehouse_log::interpolate_now_tokens(&out);
    out
}

/// True when the handler's declared `event_ref` (e.g.
/// `"Tools::ShellTool.BashRan"`) refers to the event that just fired.
/// Accepts three forms : `Context::Aggregate.Event`, `Aggregate.Event`,
/// or bare `Event`. The event itself carries `name` (the event token)
/// and `aggregate_type` (the aggregate that emitted it).
pub(super) fn event_ref_matches(event_ref: &str, event: &Event) -> bool {
    let (ctx_agg, evt_name) = match event_ref.rsplit_once('.') {
        Some((head, tail)) => (head, tail),
        None => return event_ref == event.name,
    };
    if evt_name != event.name { return false; }
    // `ctx_agg` is either `Context::Aggregate` or just `Aggregate`.
    let agg_only = ctx_agg.rsplit("::").next().unwrap_or(ctx_agg);
    if agg_only != event.aggregate_type { return false; }
    // Realm/context enforcement — symmetric with the command resolver (slice 3),
    // the SAME shared heki helper. A realm-qualified event ref must match the
    // EMITTER's stamped realm_path, carried on the event itself. Lenient : a
    // 2-seg ref or an unstamped emitter passes. So two same-named aggregates in
    // different realms no longer cross-fire each other's events.
    let (realm, context) = crate::heki::fqn_realm_context(event_ref);
    crate::heki::realm_context_matches(event.realm_path.as_deref(), realm.as_deref(), context.as_deref())
}

/// Convert the parser's source-token attrs (string values still carry
/// surrounding quotes, ints stay as digit strings) into the runtime's
/// dynamic Value form. Mirrors behaviors_runner's `parse_value`.
pub(super) fn build_attr_map(attrs: &[(String, String)]) -> HashMap<String, Value> {
    let mut out = HashMap::new();
    for (k, raw) in attrs {
        let v = raw.trim();
        if let Ok(n) = v.parse::<i64>() {
            out.insert(k.clone(), Value::Int(n));
        } else if v == "true" {
            out.insert(k.clone(), Value::Bool(true));
        } else if v == "false" {
            out.insert(k.clone(), Value::Bool(false));
        } else {
            // Strip surrounding quotes when present (string literal form).
            let s = if v.starts_with('"') && v.ends_with('"') && v.len() >= 2 {
                v[1..v.len() - 1].to_string()
            } else {
                v.to_string()
            };
            out.insert(k.clone(), Value::Str(s));
        }
    }
    out
}

/// Sprint 14 — pick the wrapped-call return value source for a fire.
/// World binding wins (when `.world` declares an adapter binding for
/// this adapter's name) ; canned is the memory-default (when the
/// `driven on` handler declares a `canned do ... end` block) ;
/// neither = empty. Factored out of `resolve_driven_adapters` so the
/// canned-vs-real switch is unit-testable without booting a Runtime.
pub fn pick_wrapped_values(
    world: Option<&[(String, String)]>,
    canned: Option<&crate::hecksagon_ir::CannedResponse>,
) -> Vec<(String, String)> {
    if let Some(w) = world { return w.to_vec(); }
    if let Some(c) = canned { return c.values.clone(); }
    Vec::new()
}

/// Sprint 14 — merge a fire's wrapped-call values with the declared
/// dispatch attrs. Wrapped values fill the slot the adapter's wrapped
/// call would produce ; declared attrs override on key collision so a
/// literal `dispatch "Y", id: "fixed"` pins `id` regardless of what
/// canned/world declared. Factored out of `resolve_driven_adapters`
/// so the override semantics are unit-testable.
pub fn merge_wrapped_and_declared(
    wrapped: &[(String, String)],
    declared: &[(String, String)],
) -> HashMap<String, Value> {
    let mut out = build_attr_map(wrapped);
    for (k, v) in build_attr_map(declared) {
        out.insert(k, v);
    }
    out
}


/// i221-C — resolve a sweep's query inputs against the triggering event
/// into a string attr map the query executor filters on.
pub(super) fn sweep_query_attrs(
    inputs: &[(String, crate::ir::ValueSpec)],
    event: &Event,
) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for (k, spec) in inputs {
        let v = match spec {
            crate::ir::ValueSpec::Literal { value } => {
                // strip the source-token quotes before interpolating so the
                // filter value matches the (unquoted) stored field.
                interpolate_event(value.trim().trim_matches('"'), event)
            }
            crate::ir::ValueSpec::FromEvent { name, default } => event
                .data
                .get(name)
                .map(val_str)
                .or_else(|| default.clone())
                .unwrap_or_default(),
            _ => String::new(),
        };
        out.insert(k.clone(), v);
    }
    out
}

/// i221-C — interpolate `{field}` tokens record-first (the swept record's
/// fields win), then fall back to the triggering event.
pub(super) fn interpolate_event_and_record(
    template: &str,
    event: &Event,
    record: &HashMap<String, Value>,
) -> String {
    let mut out = template.to_string();
    for (k, v) in record.iter() {
        out = out.replace(&format!("{{{}}}", k), &val_str(v));
    }
    interpolate_event(&out, event)
}
