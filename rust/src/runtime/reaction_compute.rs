//! reaction_compute — the `:compute` adapter family of Phase-3 reaction
//! delivery : sugar (resolve_compute_adapters) over the
//! `Primitive::Compute.Invoke` primitive, the excluded-set chain resolver,
//! the primitive-shape resolver, and the invoke engine
//! (run_compute_invoke). Inherent `impl Runtime` methods in a child module,
//! same contract as reaction.rs.
//!
//! Cask extracted VERBATIM from runtime/reaction.rs (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/reaction_compute.rs — hand-written
//!  runtime kernel-floor, relocated verbatim from reaction.rs blanket.]

use super::*;
use std::collections::HashMap;

impl Runtime {
    /// `:compute` adapter resolver — the HECKSAGON-FIRST local-function edge,
    /// SUGAR over the `Primitive::Compute.Invoke` primitive exactly as `:exec`
    /// is sugar over `Process.Spawn`. The hecksagon DECLARES the edge
    /// (`adapter :compute, function:, trigger_on:, response_into:`) ; the one
    /// compute primitive RUNS it. Replaces the bespoke
    /// `resolve_compute_adapters_with_excluded` resolver retired from
    /// runtime/mod.rs (shrink-mod phase A — the protocol is declared in
    /// primitive.bluebook, only the compute_dispatcher leaf stays imperative).
    /// Chains are preserved : a matched adapter's cascade result re-enters this
    /// resolver, bounded by the `fired` exclude-set exactly as before.
    pub(super) fn resolve_compute_adapters(
        &mut self,
        result: &CommandResult,
        command_name: &str,
    ) {
        let mut fired: std::collections::HashSet<String> = std::collections::HashSet::new();
        self.resolve_compute_adapters_excluded(result, command_name, &mut fired);
    }

    fn resolve_compute_adapters_excluded(
        &mut self,
        result: &CommandResult,
        command_name: &str,
        fired: &mut std::collections::HashSet<String>,
    ) {
        if self.hecksagons.is_empty() {
            return;
        }
        let bare_command = command_name.rsplit('.').next().unwrap_or(command_name);
        let target = format!("{}.{}", result.aggregate_type, bare_command);
        let adapters: Vec<crate::hecksagon_ir::ComputeAdapter> = self
            .hecksagons
            .iter()
            .flat_map(|h| h.compute_adapters.iter())
            .filter(|ca| ca.effective_trigger() == Some(target.as_str()))
            .filter(|ca| !fired.contains(&ca.name))
            .cloned()
            .collect();
        if adapters.is_empty() {
            return;
        }
        // The function's attrs are the upstream aggregate's state fields —
        // the same projection the retired resolver built.
        let attrs = self.state_attrs(&result.aggregate_type, &result.aggregate_id);
        for adapter in &adapters {
            fired.insert(adapter.name.clone());
            let inner = self.run_compute_invoke(
                &adapter.name,
                &adapter.function_name,
                &attrs,
                adapter.response_into_target.as_deref(),
                adapter.response_into_attr.as_deref(),
                &result.aggregate_type,
                &result.aggregate_id,
            );
            // Recurse so a chain of :compute adapters (A's cascade target
            // triggering B) fires end-to-end before the LLM hook lands —
            // same contract as the retired resolver, bounded by `fired`.
            if let Some(inner_result) = inner {
                let chained = adapter.response_into_target.clone().unwrap_or_default();
                self.resolve_compute_adapters_excluded(&inner_result, &chained, fired);
            }
        }
    }

    /// The generic `Compute.Invoke` primitive hook — sibling of
    /// `resolve_primitive_spawn`. Fires when a `Compute.Invoke` command is
    /// dispatched (a bluebook policy or the :compute sugar above composed it) :
    /// consults the PrimitiveRegistry (audit trail), reads the declared
    /// signature (function_name / response_into_target / response_into_attr —
    /// matching the seeded Storehouse::Primitive record), and runs the invoke
    /// through the one engine below. The bluebook IS the contract — anything
    /// that dispatches `Compute.Invoke` runs the function.
    pub(super) fn resolve_primitive_compute(
        &mut self,
        result: &CommandResult,
        command_name: &str,
        dispatch_attrs: &HashMap<String, Value>,
    ) {
        let bare_command = command_name.rsplit('.').next().unwrap_or(command_name);
        if result.aggregate_type != "Compute" || bare_command != "Invoke" {
            return;
        }
        let registry_key = format!("{}.{}", result.aggregate_type, bare_command);
        self.log_primitive_registry_route(&registry_key);
        let Some(function) =
            Self::required_primitive_attr("compute", dispatch_attrs, "function_name")
        else {
            return;
        };
        let result_into = dispatch_attrs.get("response_into_target").map(|v| v.to_string());
        let result_attr = dispatch_attrs.get("response_into_attr").map(|v| v.to_string());
        let invocation_id = dispatch_attrs
            .get("id")
            .map(|v| v.to_string())
            .unwrap_or_else(|| result.aggregate_id.clone());
        // The function's attrs are the dispatch attrs, string-shaped.
        let fn_attrs: HashMap<String, String> = dispatch_attrs
            .iter()
            .map(|(k, v)| (k.clone(), v.to_string()))
            .collect();
        let _ = self.run_compute_invoke(
            "compute_invoke",
            &function,
            &fn_attrs,
            result_into.as_deref(),
            result_attr.as_deref(),
            &result.aggregate_type,
            &invocation_id,
        );
    }

    /// The ONE compute engine both paths share — the sugar (hecksagon
    /// `:compute` bindings) and the primitive hook (direct `Compute.Invoke`
    /// dispatches) meet here. Resolves the function through the kernel-floor
    /// `compute_dispatcher` leaf (reused unchanged — the only imperative Rust
    /// on this path), then chains the returned text into the response target
    /// via `dispatch_cascade` so depth + cycle protection apply. Skipped
    /// outcomes (unset / unregistered function) fire no cascade, and a
    /// missing / malformed target skips the chain after the function ran —
    /// both exactly the retired resolver's contract.
    #[allow(clippy::too_many_arguments)]
    fn run_compute_invoke(
        &mut self,
        adapter_name: &str,
        function_name: &str,
        fn_attrs: &HashMap<String, String>,
        result_into: Option<&str>,
        result_attr: Option<&str>,
        upstream_type: &str,
        upstream_id: &str,
    ) -> Option<CommandResult> {
        let adapter = crate::hecksagon_ir::ComputeAdapter {
            name: adapter_name.to_string(),
            function_name: function_name.to_string(),
            trigger_on: None,
            response_into_target: result_into.map(|s| s.to_string()),
            response_into_attr: result_attr.map(|s| s.to_string()),
        };
        let state_clone: Option<AggregateState> =
            self.find(upstream_type, upstream_id).cloned();
        let data_dir = self.data_dir.clone();
        let outcome = compute_dispatcher::call(
            &adapter,
            state_clone.as_ref(),
            fn_attrs,
            data_dir.as_deref(),
        );
        let result_data = match outcome {
            compute_dispatcher::ComputeOutcome::Completed(r) => r,
            compute_dispatcher::ComputeOutcome::Skipped(_) => return None,
        };
        let (target_agg, target_cmd) = adapter
            .response_into_target
            .as_deref()
            .and_then(compute_dispatcher::split_target)?;
        let attr_name = match adapter.response_into_attr.as_deref() {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => return None,
        };
        let mut chain_attrs: HashMap<String, Value> = HashMap::new();
        chain_attrs.insert(attr_name, Value::Str(result_data.response_text.clone()));
        let cmd_qualified = format!("{}.{}", target_agg, target_cmd);
        let cascade_outcome = command_dispatch::dispatch_cascade(
            self,
            &cmd_qualified,
            chain_attrs,
            upstream_type,
            upstream_id,
        );
        storehouse_log::cascade_step(
            &cmd_qualified,
            upstream_id,
            cascade_outcome.is_ok(),
        );
        cascade_outcome.ok()
    }

}
