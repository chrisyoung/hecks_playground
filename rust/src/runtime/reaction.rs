//! Runtime Phase-3 reaction delivery — run the impure adapter edge + the
//! policy/PM/driven cascade for a dispatched command result, and drain the
//! reaction outbox to quiescence.
//!
//! Inherent `impl Runtime` methods in a child module ; they reach Runtime's
//! private `resolve_*` / `drain_policies` helpers + the `outbox` field and the
//! pub `driven_adapter_resolver` module through child-module privacy + the
//! `super::*` glob. `react` / `react_ports` are `pub(super)` because mod.rs's
//! dispatch paths call them.
//!
//! [antibody-exempt: rust/src/runtime/reaction.rs — hand-written runtime
//!  kernel-floor. Was a codegen/runtime_shape artifact ; the shape was retired
//!  2026-06-27 — a .bluebook that only re-emitted imperative Rust captures no
//!  domain, so the runtime kernel is hand-maintained Rust like mod.rs.]

use super::*;
use std::collections::HashMap;

impl Runtime {
    /// Phase 3 — run every reaction for one dispatched command result:
    /// the compute/policy/PM cascade, the IO-edge adapter hooks, and the
    /// hecksagon `driven on` adapters. Invoked ONLY by `pump()`, never
    /// inline in the core dispatch — so the mutation step stays single-
    /// aggregate and the reactions are a separate phase.
    pub(super) fn react(
        &mut self,
        result: &CommandResult,
        command_name: &str,
        attrs: &HashMap<String, Value>,
    ) {
        self.resolve_compute_adapters(result, command_name);
        self.drain_policies(result);
        self.resolve_llm_adapters(result, command_name);
        self.resolve_claude_tool_adapters(result, command_name, attrs);
        self.resolve_mcp_adapters(result, command_name, attrs);
        #[cfg(not(target_arch = "wasm32"))]
        self.resolve_web_tool_adapters(result, command_name, attrs);
        self.resolve_primitive_compute(result, command_name, attrs);
        self.resolve_primitive_mcp(result, command_name, attrs);
        self.resolve_primitive_claude_tool(result, command_name, attrs);
        self.resolve_primitive_llm(result, command_name, attrs);
        self.resolve_primitive_spawn(result, command_name, attrs, None);
        self.resolve_exec_adapters(result, command_name, attrs);
        if let Some(ref event) = result.event {
            let event_clone = event.clone();
            driven_adapter_resolver::resolve_driven_adapters(self, &event_clone);
        }
    }

    /// C3 cutover — the IMPURE hexagonal edge: adapters that talk to the
    /// outside world (compute / llm / claude_tool / mcp / web / process-spawn).
    /// These fire EAGERLY even on the deferred path — they are ports
    /// (a synchronous request/response at the boundary), not aggregate-to-
    /// aggregate domain reactions. Order matches react()'s adapter prefix.
    pub(super) fn react_ports(
        &mut self,
        result: &CommandResult,
        command_name: &str,
        attrs: &HashMap<String, Value>,
    ) {
        self.resolve_compute_adapters(result, command_name);
        self.resolve_llm_adapters(result, command_name);
        self.resolve_claude_tool_adapters(result, command_name, attrs);
        self.resolve_mcp_adapters(result, command_name, attrs);
        #[cfg(not(target_arch = "wasm32"))]
        self.resolve_web_tool_adapters(result, command_name, attrs);
        self.resolve_primitive_compute(result, command_name, attrs);
        self.resolve_primitive_mcp(result, command_name, attrs);
        self.resolve_primitive_claude_tool(result, command_name, attrs);
        self.resolve_primitive_llm(result, command_name, attrs);
        self.resolve_primitive_spawn(result, command_name, attrs, None);
        self.resolve_exec_adapters(result, command_name, attrs);
    }

    /// `:exec` adapter resolver — the HECKSAGON-FIRST impure edge that runs a
    /// program when a command completes. Sibling of the other react_ports
    /// resolvers, but routes through the SINGLE `Process.Spawn` primitive
    /// (`resolve_primitive_spawn`) rather than a bespoke per-family executor :
    /// `adapter :exec, command:, exec:, result_into:` is SUGAR over Process.Spawn.
    /// The hecksagon DECLARES the impure edge ; the one spawn primitive RUNS it.
    /// Matches every `:exec` IoAdapter whose `command:` option equals the just-
    /// dispatched `Aggregate.Command`, firing the spawn with cmd=exec,
    /// result_into, and the originating invocation id as the cascade join key.
    /// Restores the pre-bacc0692f hecksagon surface WITHOUT the retired
    /// `resolve_exec_adapters` bespoke executor (the spawn is the one primitive).
    fn resolve_exec_adapters(
        &mut self,
        result: &CommandResult,
        command_name: &str,
        _dispatch_attrs: &HashMap<String, Value>,
    ) {
        if self.hecksagons.is_empty() {
            return;
        }
        // The command portion is everything after "{aggregate_type}." in the
        // dispatched command_name — preserving ENTITY-SCOPED command names
        // (File.Move, Directory.Remove) so they do not collide on a bare last
        // segment (File.Remove + Directory.Remove both end ".Remove"). Falls
        // back to the last segment when the aggregate prefix is absent.
        let agg_prefix = format!("{}.", result.aggregate_type);
        let command_part = command_name
            .rfind(&agg_prefix)
            .map(|i| &command_name[i + agg_prefix.len()..])
            .unwrap_or_else(|| command_name.rsplit('.').next().unwrap_or(command_name));
        let target = format!("{}.{}", result.aggregate_type, command_part);
        // NOTE : exec matches the FULL binding command (`c == target`), not
        // binding_command_tail — preserved from the pre-primitive surface.
        let matched: Vec<(String, Option<String>)> = self
            .matched_io_bindings("exec", &target, false, &["exec", "result_into"])
            .into_iter()
            .filter_map(|mut v| v[0].take().map(|e| (e, v[1].take())))
            .collect();
        if matched.is_empty() {
            return;
        }
        let invocation_id = self
            .find(&result.aggregate_type, &result.aggregate_id)
            .and_then(|s| s.fields.get("id").map(|v| v.to_string()))
            .unwrap_or_else(|| result.aggregate_id.clone());
        for (exec, result_into) in &matched {
            let mut attrs: HashMap<String, Value> = HashMap::new();
            attrs.insert("cmd".to_string(), Value::Str(exec.clone()));
            if let Some(ri) = result_into {
                if !ri.is_empty() {
                    attrs.insert("result_into".to_string(), Value::Str(ri.clone()));
                }
            }
            attrs.insert("id".to_string(), Value::Str(invocation_id.clone()));
            let synth = CommandResult {
                aggregate_id: invocation_id.clone(),
                aggregate_type: "Process".to_string(),
                event: None,
                deltas: Vec::new(),
            };
            self.resolve_primitive_spawn(&synth, "Process.Spawn", &attrs, None);
        }
    }

    /// The ONE tool-outcome cascade tail every tool-shaped primitive shares
    /// (mcp, claude_tool — the same (id, tool, output, exit_code, ok) record
    /// the retired resolvers each hand-built). Routes through
    /// `dispatch_cascade` so depth + cycle protection apply, logs the cascade
    /// step, and swallows the outcome (failures are observable via the debug
    /// line ; a tool-side error never breaks the original dispatch's return).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn cascade_tool_outcome(
        &mut self,
        family: &str,
        result_into_target: &str,
        invocation_id: &str,
        tool: &str,
        output: String,
        exit_code: i64,
        ok: bool,
        upstream_type: &str,
        upstream_id: &str,
        debug: bool,
    ) {
        let mut record_attrs: HashMap<String, Value> = HashMap::new();
        record_attrs.insert("id".to_string(), Value::Str(invocation_id.to_string()));
        record_attrs.insert("tool".to_string(), Value::Str(tool.to_string()));
        record_attrs.insert("output".to_string(), Value::Str(output));
        record_attrs.insert("exit_code".to_string(), Value::Int(exit_code));
        record_attrs.insert("ok".to_string(), Value::Bool(ok));
        let cascade_outcome = command_dispatch::dispatch_cascade(
            self,
            result_into_target,
            record_attrs,
            upstream_type,
            upstream_id,
        );
        storehouse_log::cascade_step(
            result_into_target,
            invocation_id,
            cascade_outcome.is_ok(),
        );
        if debug {
            match &cascade_outcome {
                Ok(_) => eprintln!("[{}:debug] cascaded into {} ok", family, result_into_target),
                Err(e) => eprintln!(
                    "[{}:debug] cascade into {} failed: {:?}",
                    family, result_into_target, e
                ),
            }
        }
        let _ = cascade_outcome;
    }

    /// Phase 3 — deliver enqueued reactions. Each pending reaction runs
    /// in a phase separate from the command that produced it. Follow-on
    /// commands dispatched inside `react` (policy/PM cascade, driven
    /// adapters) still cascade synchronously via `dispatch_cascade` for
    /// now; flattening those into the outbox too is the next step. The
    /// loop drains to quiescence so eager callers see a settled cascade.
    pub fn pump(&mut self) {
        while let Some(p) = self.outbox.pop_front() {
            self.react(&p.result, &p.command_name, &p.attrs);
        }
    }

}
