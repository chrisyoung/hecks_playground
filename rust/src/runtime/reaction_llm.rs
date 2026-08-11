//! reaction_llm — the `:llm` adapter family of Phase-3 reaction delivery :
//! sugar (resolve_llm_adapters) over the `Primitive::Llm.Invoke` primitive
//! with cross-aggregate attr scaffolding (i218 prefixed {{Agg_field}} +
//! bare {{field}}), the KEYSTONE effect-binding switch, the fired-set chain
//! recursion (gap3/i220-3), the primitive-shape resolver, and the invoke
//! engine (run_llm_invoke). Inherent `impl Runtime` methods in a child
//! module, same contract as reaction.rs.
//!
//! Cask extracted VERBATIM from runtime/reaction.rs (cask-runtime) ; the
//! sugar's 12-line doc compressed under the licensed doc-header trim.
//!
//! [antibody-exempt: rust/src/runtime/reaction_llm.rs — hand-written
//!  runtime kernel-floor, relocated verbatim from reaction.rs blanket.]

use super::*;
use std::collections::HashMap;

impl Runtime {
    /// `:llm` adapter resolver — hecksagon-first sugar over `Llm.Invoke` :
    /// composes {{Agg_field}}/{{field}} scaffolding, honours the KEYSTONE
    /// effect-binding switch + fired-set chain recursion (see module header).
    pub(super) fn resolve_llm_adapters(&mut self, result: &CommandResult, command_name: &str) {
        let mut fired: std::collections::HashSet<String> = std::collections::HashSet::new();
        self.resolve_llm_adapters_excluded(result, command_name, &mut fired);
    }

    fn resolve_llm_adapters_excluded(
        &mut self,
        result: &CommandResult,
        command_name: &str,
        fired: &mut std::collections::HashSet<String>,
    ) {
        // KEYSTONE SWITCH (slice 2) — shape-derived, NOT env-gated. When an
        // effect binding subscribes (`on:`) to the EVENT this command emitted
        // and resolves to the `llm` family with a verdict, the OUT-OF-PROCESS
        // path owns llm completion ; suppress the in-process path so the two
        // never double-produce the verdict.
        if let Some(ref event) = result.event {
            if self.has_effect_binding_for(&event.name, "llm") {
                return;
            }
        }
        let debug_llm = std::env::var("HECKS_DEBUG_LLM").is_ok();
        if self.hecksagons.is_empty() {
            if debug_llm {
                eprintln!("[llm:debug] no hecksagons — returning");
            }
            return;
        }
        let bare_command = command_name.rsplit('.').next().unwrap_or(command_name);
        let target = format!("{}.{}", result.aggregate_type, bare_command);
        let adapters: Vec<crate::hecksagon_ir::LlmAdapter> = self
            .hecksagons
            .iter()
            .flat_map(|h| h.llm_adapters.iter())
            .filter(|la| la.effective_trigger() == Some(target.as_str()))
            .filter(|la| !fired.contains(&la.name))
            .cloned()
            .collect();
        if debug_llm {
            eprintln!(
                "[llm:debug] target={} matched={} fired={}",
                target,
                adapters.len(),
                fired.len()
            );
        }
        if adapters.is_empty() {
            return;
        }
        let state_clone: Option<AggregateState> = self
            .find(&result.aggregate_type, &result.aggregate_id)
            .cloned();
        // Composition (i218 cross-aggregate scaffolder) : every aggregate's
        // latest state under prefixed keys ({{Dream_text_fr}}) plus bare keys
        // for cross-aggregate references (last-write-wins) ; the dispatched
        // aggregate's own fields then overwrite the bare names — the most
        // specific source wins.
        let mut attrs: HashMap<String, String> = HashMap::new();
        let agg_names: Vec<String> = self.repositories.keys().cloned().collect();
        for agg_name in &agg_names {
            let states: Vec<AggregateState> = self
                .repositories
                .get(agg_name)
                .map(|r| r.all().into_iter().cloned().collect())
                .unwrap_or_default();
            if let Some(latest) = states.last() {
                for (k, v) in &latest.fields {
                    attrs.insert(format!("{}_{}", agg_name, k), v.to_string());
                    if agg_name != &result.aggregate_type {
                        attrs.entry(k.clone()).or_insert_with(|| v.to_string());
                    }
                }
            }
        }
        if let Some(s) = state_clone.as_ref() {
            for (k, v) in &s.fields {
                attrs.insert(k.clone(), v.to_string());
            }
        }
        for adapter in &adapters {
            // Pre-mark so a recursive resolve sees it — the historical
            // trigger==response shape must fire exactly once per tree.
            fired.insert(adapter.name.clone());
            let inner = self.run_llm_invoke(
                adapter,
                state_clone.as_ref(),
                &attrs,
                &result.aggregate_type,
                &result.aggregate_id,
            );
            // gap3 (i220-3) — chain LLM resolution on the response cascade so
            // a second adapter triggered by the response-target command fires
            // too, bounded by `fired`.
            if let Some(inner_result) = inner {
                let chained = adapter.response_into_target.clone().unwrap_or_default();
                self.resolve_llm_adapters_excluded(&inner_result, &chained, fired);
            }
        }
    }

    /// The generic `Llm.Invoke` primitive hook (sibling of the other
    /// resolve_primitive_* hooks) : PrimitiveRegistry + declared signature
    /// (FINAL prompt — substitution is the sugar's job) → the one engine
    /// below. The bluebook IS the contract.
    pub(super) fn resolve_primitive_llm(
        &mut self,
        result: &CommandResult,
        command_name: &str,
        dispatch_attrs: &HashMap<String, Value>,
    ) {
        let bare_command = command_name.rsplit('.').next().unwrap_or(command_name);
        if result.aggregate_type != "Llm" || bare_command != "Invoke" {
            return;
        }
        let registry_key = format!("{}.{}", result.aggregate_type, bare_command);
        self.log_primitive_registry_route(&registry_key);
        let Some(prompt) = Self::required_primitive_attr("llm", dispatch_attrs, "prompt") else {
            return;
        };
        let adapter = crate::hecksagon_ir::LlmAdapter {
            name: "llm_invoke".to_string(),
            prompt_template: prompt,
            model: dispatch_attrs.get("model").map(|v| v.to_string()).filter(|s| !s.is_empty()),
            max_tokens: None,
            trigger_on: None,
            response_into_target: dispatch_attrs
                .get("response_into_target")
                .map(|v| v.to_string())
                .filter(|s| !s.is_empty()),
            response_into_attr: dispatch_attrs
                .get("response_into_attr")
                .map(|v| v.to_string())
                .filter(|s| !s.is_empty()),
            backend: dispatch_attrs
                .get("backend")
                .map(|v| v.to_string())
                .filter(|s| !s.is_empty()),
        };
        let fn_attrs: HashMap<String, String> = dispatch_attrs
            .iter()
            .map(|(k, v)| (k.clone(), v.to_string()))
            .collect();
        let _ = self.run_llm_invoke(
            &adapter,
            None,
            &fn_attrs,
            &result.aggregate_type,
            &result.aggregate_id,
        );
    }

    /// The ONE llm engine both paths share. Borrows the provider registry,
    /// runs the kernel-floor `llm_dispatcher` leaf — reused unchanged, the
    /// only imperative Rust on this path — then chains the response text into
    /// response_into_target(response_into_attr) via `dispatch_cascade`.
    /// Skipped outcomes and missing / malformed targets fire no cascade —
    /// exactly the retired resolver's contract. Returns the inner result so
    /// the sugar can chain adapter-on-response recursion.
    fn run_llm_invoke(
        &mut self,
        adapter: &crate::hecksagon_ir::LlmAdapter,
        state: Option<&AggregateState>,
        attrs: &HashMap<String, String>,
        upstream_type: &str,
        upstream_id: &str,
    ) -> Option<CommandResult> {
        let outcome = {
            let providers = &self.llm_providers;
            let providers_arg = if providers.is_empty() { None } else { Some(providers) };
            llm_dispatcher::call(adapter, state, attrs, providers_arg)
        };
        let result_data = match outcome {
            llm_dispatcher::LlmOutcome::Completed(r) => r,
            llm_dispatcher::LlmOutcome::Skipped(_) => return None,
        };
        let (target_agg, target_cmd) = adapter
            .response_into_target
            .as_deref()
            .and_then(llm_dispatcher::split_target)?;
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
        cascade_outcome.ok()
    }
}
