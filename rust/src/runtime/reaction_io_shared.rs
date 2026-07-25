//! reaction_io_shared — the plumbing every io-adapter family shares :
//! matched_io_bindings (the ONE binding-match loop), state_attrs (upstream
//! aggregate state as string attrs), required_primitive_attr (signature
//! field extraction), and log_primitive_registry_route (the audit line).
//! Consumed by the reaction_compute / _mcp / _claude_tool / _llm casks and
//! the exec resolver in reaction.rs. Inherent `impl Runtime` methods in a
//! child module, same contract as reaction.rs.
//!
//! Cask extracted VERBATIM from runtime/reaction.rs (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/reaction_io_shared.rs — hand-written
//!  runtime kernel-floor, relocated verbatim from reaction.rs blanket.]

use super::*;
use std::collections::HashMap;

impl Runtime {
    /// Match every `io_adapter` binding of `kind` whose `command:` option
    /// equals the dispatched target (via `binding_command_tail` when
    /// `tail_match`, else the full string — exec's historical form),
    /// returning the requested option values (quote/colon-stripped),
    /// positionally by `keys`. The ONE match loop every io-family sugar
    /// shares.
    pub(super) fn matched_io_bindings(
        &self,
        kind: &str,
        target: &str,
        tail_match: bool,
        keys: &[&str],
    ) -> Vec<Vec<Option<String>>> {
        self.hecksagons
            .iter()
            .flat_map(|h| h.io_adapters.iter())
            .filter(|a| a.kind == kind)
            .filter_map(|a| {
                let mut cmd: Option<String> = None;
                let mut vals: Vec<Option<String>> = vec![None; keys.len()];
                for (k, v) in &a.options {
                    if k == "command" {
                        cmd = Some(strip_quotes_or_colon(v));
                    } else if let Some(i) = keys.iter().position(|kk| *kk == k.as_str()) {
                        vals[i] = Some(strip_quotes_or_colon(v));
                    }
                }
                let hit = match &cmd {
                    Some(c) if tail_match => binding_command_tail(c) == target,
                    Some(c) => c == target,
                    None => false,
                };
                if hit { Some(vals) } else { None }
            })
            .collect()
    }

    /// The upstream aggregate's state fields, string-shaped — the base attrs
    /// projection every sugar composes from.
    pub(super) fn state_attrs(&self, aggregate_type: &str, aggregate_id: &str) -> HashMap<String, String> {
        let mut attrs: HashMap<String, String> = HashMap::new();
        if let Some(s) = self.find(aggregate_type, aggregate_id) {
            for (k, v) in &s.fields {
                attrs.insert(k.clone(), v.to_string());
            }
        }
        attrs
    }

    /// Required dispatch attr for a primitive hook — present-and-non-empty
    /// or a loud skip line, the guard every hook shares.
    pub(super) fn required_primitive_attr(
        family: &str,
        attrs: &HashMap<String, Value>,
        key: &str,
    ) -> Option<String> {
        match attrs.get(key).map(|v| v.to_string()) {
            Some(v) if !v.is_empty() => Some(v),
            _ => {
                println!(
                    "[{}] [primitive:{}] skipped — missing {} attr",
                    storehouse_log::now_iso8601(),
                    family,
                    key,
                );
                None
            }
        }
    }

    /// PrimitiveRegistry audit line every resolve_primitive_* hook shares —
    /// routed when the key has a Storehouse::Primitive declaration, a loud
    /// miss when it does not (the macrophage's signal that an imperative
    /// leaf lacks its declaration).
    pub(super) fn log_primitive_registry_route(&self, registry_key: &str) {
        match self.primitive_registry.lookup(registry_key) {
            Some(spec) => println!(
                "[{}] [primitive:registry] routed name={} kind={} impl={}",
                storehouse_log::now_iso8601(),
                spec.name, spec.kind, spec.implementation,
            ),
            None => println!(
                "[{}] [primitive:registry] miss name={} — Storehouse::Primitive declaration absent",
                storehouse_log::now_iso8601(),
                registry_key,
            ),
        }
    }
}
