//! Sprint 14 first-adapter slice — fires `driven on` adapters declared
//! [antibody-exempt: kernel-floor runtime — spawns driven-adapter run-leaves in-process ; the engine that executes hecksagon adapters, inherently Rust]//! in `<bluebook>/hecksagons/<service>.hecksagon`.
//!
//! Shape:
//!
//! ```text
//! adapter "ShellAdapter" do
//!   driven on "Tools::ShellTool.BashRan" do |event|
//!     canned do
//!       output "ack"
//!       exit_code 0
//!     end
//!     dispatch "Tools::TaskTool.Get", id: "shell-adapter-smoke"
//!   end
//! end
//! ```
//!
//! The parser captures these as `DrivenAdapter` IR entries on each
//! attached `Hecksagon`. This resolver runs after `Runtime::dispatch`
//! (and `Runtime::dispatch_isolated`) publishes an event : it scans
//! every attached hecksagon for handlers whose `event_ref` matches
//! the just-emitted event, then dispatches each handler's follow-on
//! commands through `command_dispatch::dispatch_cascade`.
//!
//! Sprint 14 memory-canned-defaults + world-wires-real-adapters — the
//! resolver picks the wrapped-call return value for each fire :
//!
//!   - `.world` declares `adapter "Name" do; <key> <value> end`
//!     → use the binding's values ("real backend with config from
//!     that entry")
//!   - no `.world` entry, but the handler has `canned do ... end`
//!     → use the canned values (memory + canned default)
//!   - neither → dispatch only the declared static attrs (legacy)
//!
//! In all three cases the chosen value-source is merged into the
//! follow-on dispatch's attr map ; declared dispatch attrs win on
//! conflict (a literal `dispatch "Y", id: "fixed"` overrides any
//! canned/world `id`). Presence of a `.world` adapter entry IS the
//! signal that switches canned→real ; there is no `backend:` flag
//! and no fake-vs-real adapter distinction. The adapter declaration
//! is identical across deployments ; only `.world` differs.
//!
//! Sibling of `resolve_claude_tool_adapters` / `resolve_web_tool_adapters`
//! / `resolve_mcp_adapters` — same registration shape, but triggered by
//! event emission rather than command name. The follow-on uses the
//! cascade path so the second dispatch's emit lands on the event bus and
//! `expect emits: [...]` assertions see the full chain.
//!
//! Cycle protection : the resolver is wired into `dispatch` and
//! `dispatch_isolated` only, NOT into `dispatch_cascade`. A follow-on
//! that itself emits an event won't recursively fire driven adapters —
//! the cascade path is one-shot per driven trigger. (Mirror of the same
//! rule applied to `resolve_claude_tool_adapters`.)

use std::collections::HashMap;
use super::driven_adapter_args::*;
pub use super::driven_adapter_args::{merge_wrapped_and_declared, pick_wrapped_values};
pub use super::driven_adapter_enumerate::enumerate_driven_dispatches;
use crate::runtime::{Runtime, Value, command_dispatch};
use crate::runtime::event_bus::Event;

/// Scan attached hecksagons for `driven on` handlers matching the
/// just-published event, and dispatch each declared follow-on via the
/// cascade path. No-op when no hecksagons are attached (the historical
/// `Runtime::boot` path), no driven adapters declared, or no handler
/// matches the event.
/// Maximum cascade-resolver recursion depth. A driven adapter whose follow-on
/// emits an event that triggers ANOTHER driven adapter chains automatically
/// (e.g. Claim.Acquire → Lease.Grant → worktree_create). This bound
/// hard-stops any cyclic adapter graph (A→B→A…) : cycle protection IS the
/// depth guard, because dispatch_inner carries no counter of its own.
const MAX_CASCADE_DEPTH: usize = 16;

pub fn resolve_driven_adapters(rt: &mut Runtime, event: &Event) {
    resolve_driven_adapters_at_depth(rt, event, 0);
}

fn resolve_driven_adapters_at_depth(rt: &mut Runtime, event: &Event, depth: usize) {
    if depth >= MAX_CASCADE_DEPTH {
        eprintln!("[driven] cascade depth {} reached at event '''{}''' — stopping (cycle guard)", MAX_CASCADE_DEPTH, event.name);
        return;
    }
    if rt.hecksagons.is_empty() { return; }
    let debug = std::env::var("HECKS_DEBUG_DRIVEN").is_ok();

    // Snapshot matching fires up-front. The follow-on dispatch takes
    // `&mut Runtime`, so we can't keep an immutable borrow of
    // `rt.hecksagons` / `rt.world_adapter_bindings` open across the
    // call. Clone is cheap : driven adapter declarations are small
    // static IR. For each fire we capture (command, declared_attrs,
    // wrapped_call_values) so the merge happens once at the snapshot
    // boundary — the resolver doesn't re-walk the world list per
    // dispatch.
    // (adapter kind, its options, its on_events) — one matched driven adapter.
    type MatchedAdapter = (String, Vec<(String, String)>, Vec<(String, String)>);
    let mut matched: Vec<MatchedAdapter> = Vec::new();
    let mut runs_to_exec: Vec<String> = Vec::new();
    let mut checks_to_run: Vec<(String, String)> = Vec::new();

        for hex in rt.hecksagons.iter() {
        for adapter in hex.driven_adapters.iter() {
            // Sprint 14 world-wires-real-adapters — presence of a
            // `.world` adapter binding with this adapter's name IS the
            // signal to use the binding's values as the wrapped-call
            // return. Absent : fall back to the handler's `canned do`
            // block. Both absent : empty merge (legacy behaviour). The
            // pick is factored into `pick_wrapped_values` so the choice
            // is unit-testable without booting a Runtime.
            let world_values: Option<Vec<(String, String)>> = rt
                .world_adapter_bindings
                .iter()
                .find(|b| b.name == adapter.name)
                .map(|b| b.values.clone());
            for handler in adapter.handlers.iter() {
                if !event_ref_matches(&handler.event_ref, event) { continue; }
                let wrapped = pick_wrapped_values(
                    world_values.as_deref(),
                    handler.canned.as_ref(),
                );
                for dispatch in handler.dispatches.iter() {
                    // i221-C — a `for_each:` clause makes the dispatch a SWEEP ;
                    // a bare dispatch keeps the single-fire path. Both interpolate
                    // `{field}`/`{id}` tokens from the event (and, for a sweep,
                    // record-first from each matched record).
                    match &dispatch.for_each {
                        None => {
                            let interp_attrs: Vec<(String, String)> = dispatch
                                .attrs
                                .iter()
                                .map(|(k, v)| (k.clone(), interpolate_event(v, event)))
                                .collect();
                            matched.push((
                                dispatch.command.clone(),
                                interp_attrs,
                                wrapped.clone(),
                            ));
                        }
                        Some(spec) => {
                            let qattrs = sweep_query_attrs(&spec.query_inputs, event);
                            for record in rt.sweep_records(spec, &qattrs) {
                                let interp_attrs: Vec<(String, String)> = dispatch
                                    .attrs
                                    .iter()
                                    .map(|(k, v)| (k.clone(),
                                        interpolate_event_and_record(v, event, &record)))
                                    .collect();
                                matched.push((
                                    dispatch.command.clone(),
                                    interp_attrs,
                                    wrapped.clone(),
                                ));
                            }
                        }
                    }
                }

                for run_cmd in handler.runs.iter() {
                    runs_to_exec.push(interpolate_event(run_cmd, event));
                }
                for check in handler.checks.iter() {
                    checks_to_run.push((interpolate_event(&check.cmd, event), check.result_into.clone()));
                }            }
        }
    }

    if matched.is_empty() && runs_to_exec.is_empty() && checks_to_run.is_empty() { return; }

    for (command, declared_attrs, wrapped_values) in matched {
        // Merge order : wrapped values first, then declared attrs
        // override on key collision. A literal `dispatch "Y", id: "x"`
        // pins `id` regardless of what canned/world declared. The
        // merge is factored into `merge_wrapped_and_declared` so the
        // override semantics are unit-testable.
        let attr_map = merge_wrapped_and_declared(&wrapped_values, &declared_attrs);
        // Cascade dispatch so the follow-on's emit reaches the bus AND
        // depth / cycle protection from the dispatch_inner stack apply.
        // Same shape as the claude_tool resolver's chain into
        // `result_into` ; the upstream id/type are the event's so the
        // cascade can preserve the invocation handle.
        let outcome = command_dispatch::dispatch_cascade(
            rt,
            &command,
            attr_map,
            &event.aggregate_type,
            &event.aggregate_id,
        );
        if debug {
            match &outcome {
                Ok(r) => eprintln!("[driven:debug] cascaded into {} ok agg={} id={}", command, r.aggregate_type, r.aggregate_id),
                Err(e) => eprintln!("[driven:debug] cascade into {} FAILED: {:?}", command, e),
            }
        }
        // resolver-on-cascade : if the follow-on emitted an event, drive
        // the next hop of the chain, bounded by MAX_CASCADE_DEPTH. This is
        // what makes a multi-adapter fleet chain run end-to-end —
        // Claim.Acquire → Lease.Grant → worktree_create — instead of stopping
        // after one hop. dispatch_cascade itself still does NOT auto-invoke
        // the resolver ; the recursion is driven HERE, under the depth guard.
        if let Ok(r) = outcome {
                // Cross adapter->policy : an adapter-dispatched command must drain
                // policies too, exactly as Runtime::dispatch does after a top-level
                // dispatch. Without this, accounting policies (e.g. BumpOnTaskReopened)
                // silently never fire when their command is dispatched from a hecksagon.
                rt.drain_policies(&r);
            if let Some(ev) = r.event {
                resolve_driven_adapters_at_depth(rt, &ev, depth + 1);
            }
        }
    }

    // Real in-process external commands declared by `run "..."` on the
    // handler. Spawned directly (argv, no shell, no bin script). This is
    // the adapter actually doing impure work — e.g. `git worktree add`.
    for cmd in runs_to_exec {
        let parts: Vec<String> = split_argv(&cmd);
        if parts.is_empty() { continue; }
        match std::process::Command::new(&parts[0]).args(&parts[1..]).output() {
            Ok(o) => eprintln!("[driven:run] {} -> exit={} stdout={} stderr={}", cmd, o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).trim(), String::from_utf8_lossy(&o.stderr).trim()),
            Err(e) => eprintln!("[driven:run] {} -> spawn FAILED: {}", cmd, e),
        }
    }
    // Live-check leaves : run each declared check in-process, arm its flag
    // via result_into with ok = (exit == 0). No bin script, no canned.
    for (cmd, target) in checks_to_run {
        let parts: Vec<String> = split_argv(&cmd);
        if parts.is_empty() { continue; }
        let ok = std::process::Command::new(&parts[0]).args(&parts[1..]).output()
            .map(|o| o.status.success()).unwrap_or(false);
        eprintln!("[driven:check] {} -> ok={} -> {}", cmd, ok, target);
        let mut attrs = HashMap::new();
        attrs.insert("ok".to_string(), Value::Bool(ok));
        let _ = command_dispatch::dispatch_cascade(rt, &target, attrs, &event.aggregate_type, &event.aggregate_id);
    }
}
