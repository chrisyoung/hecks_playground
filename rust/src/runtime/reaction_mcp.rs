//! reaction_mcp — the `:mcp` adapter family of Phase-3 reaction delivery :
//! sugar (resolve_mcp_adapters) over the `Primitive::McpTool.Invoke`
//! primitive with `{attr}` placeholder composition, the primitive-shape
//! resolver, and the invoke engine (run_mcp_invoke). The shared
//! tool-outcome cascade tail stays in reaction.rs (cascade_tool_outcome).
//! Inherent `impl Runtime` methods in a child module, same contract as
//! reaction.rs.
//!
//! Cask extracted VERBATIM from runtime/reaction.rs (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/reaction_mcp.rs — hand-written
//!  runtime kernel-floor, relocated verbatim from reaction.rs blanket.]

use super::*;
use std::collections::HashMap;

impl Runtime {
    /// `:mcp` adapter resolver — hecksagon-first sugar over `McpTool.Invoke` :
    /// composes `{attr}` placeholders (state ∪ attrs) ; the primitive RUNS it.
    pub(super) fn resolve_mcp_adapters(
        &mut self,
        result: &CommandResult,
        command_name: &str,
        dispatch_attrs: &HashMap<String, Value>,
    ) {
        if self.hecksagons.is_empty() {
            return;
        }
        let debug = std::env::var("HECKS_DEBUG_MCP").is_ok();
        let bare_command = command_name.rsplit('.').next().unwrap_or(command_name);
        let target = format!("{}.{}", result.aggregate_type, bare_command);
        let matched: Vec<(String, String, String, Option<String>)> = self
            .matched_io_bindings("mcp", &target, true, &["server", "tool", "args", "result_into"])
            .into_iter()
            .filter_map(|mut v| match (v[0].take(), v[1].take()) {
                (Some(s), Some(t)) => Some((s, t, v[2].take().unwrap_or_default(), v[3].take())),
                _ => None,
            })
            .collect();
        if debug {
            eprintln!(
                "[mcp:debug] resolve cmd={} target={} matched={}",
                command_name, target, matched.len()
            );
        }
        if matched.is_empty() {
            return;
        }
        // Composition : upstream state ∪ dispatch attrs (overlay wins).
        let mut attrs = self.state_attrs(&result.aggregate_type, &result.aggregate_id);
        for (k, v) in dispatch_attrs {
            attrs.insert(k.clone(), v.to_string());
        }
        let invocation_id = attrs
            .get("id")
            .cloned()
            .unwrap_or_else(|| result.aggregate_id.clone());
        for (server, tool, args_raw, result_into) in &matched {
            let args_value: serde_json::Value = if args_raw.is_empty() {
                serde_json::Value::Object(Default::default())
            } else {
                match serde_json::from_str(args_raw) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!(
                            "[mcp:warn] adapter for {} :args is not JSON ({}) — skipping",
                            target, e
                        );
                        continue;
                    }
                }
            };
            let args_substituted = mcp_dispatcher::substitute_value(args_value, &attrs);
            self.run_mcp_invoke(
                server,
                tool,
                &args_substituted,
                result_into.as_deref(),
                &invocation_id,
                &target,
                &result.aggregate_type,
                &result.aggregate_id,
            );
        }
    }

    /// The generic `McpTool.Invoke` primitive hook (sibling of spawn/compute) :
    /// PrimitiveRegistry + declared signature → the one engine below.
    pub(super) fn resolve_primitive_mcp(
        &mut self,
        result: &CommandResult,
        command_name: &str,
        dispatch_attrs: &HashMap<String, Value>,
    ) {
        let bare_command = command_name.rsplit('.').next().unwrap_or(command_name);
        if result.aggregate_type != "McpTool" || bare_command != "Invoke" {
            return;
        }
        let registry_key = format!("{}.{}", result.aggregate_type, bare_command);
        self.log_primitive_registry_route(&registry_key);
        let Some(server) = Self::required_primitive_attr("mcp", dispatch_attrs, "server") else {
            return;
        };
        let Some(tool) = Self::required_primitive_attr("mcp", dispatch_attrs, "tool") else {
            return;
        };
        let arguments_raw = dispatch_attrs
            .get("arguments")
            .map(|v| v.to_string())
            .unwrap_or_default();
        let args_value: serde_json::Value = if arguments_raw.is_empty() {
            serde_json::Value::Object(Default::default())
        } else {
            match serde_json::from_str(&arguments_raw) {
                Ok(v) => v,
                Err(e) => {
                    println!(
                        "[{}] [primitive:mcp] skipped — arguments is not JSON ({})",
                        storehouse_log::now_iso8601(),
                        e
                    );
                    return;
                }
            }
        };
        let result_into = dispatch_attrs.get("result_into").map(|v| v.to_string());
        let invocation_id = dispatch_attrs
            .get("id")
            .map(|v| v.to_string())
            .unwrap_or_else(|| result.aggregate_id.clone());
        let target = registry_key;
        self.run_mcp_invoke(
            &server,
            &tool,
            &args_value,
            result_into.as_deref(),
            &invocation_id,
            &target,
            &result.aggregate_type,
            &result.aggregate_id,
        );
    }

    /// The ONE mcp engine both paths share : `.world` → harness transport ;
    /// spawnable → `mcp_dispatcher` leaf ; neither → log + skip ; outcome cascades.
    #[allow(clippy::too_many_arguments)]
    fn run_mcp_invoke(
        &mut self,
        server: &str,
        tool: &str,
        args_substituted: &serde_json::Value,
        result_into: Option<&str>,
        invocation_id: &str,
        target: &str,
        upstream_type: &str,
        upstream_id: &str,
    ) {
        let debug = std::env::var("HECKS_DEBUG_MCP").is_ok();
        #[cfg(not(target_arch = "wasm32"))]
        if crate::adapter_resolution::mcp::resolve_world_server(
            self,
            server,
            tool,
            args_substituted,
            result_into,
            invocation_id,
            target,
        ) {
            return;
        }
        if !mcp_dispatcher::server_is_registered(server) {
            eprintln!(
                "[mcp:warn] adapter for {} declares server={} which is neither \
                world-declared nor a spawnable server. Dispatch recorded ; MCP call skipped.",
                target, server
            );
            return;
        }
        let tool_result = mcp_dispatcher::dispatch(server, tool, args_substituted);
        let err_tail = if tool_result.error.is_empty() {
            String::new()
        } else {
            format!(" error={:?}", tool_result.error)
        };
        eprintln!(
            "[mcp:{}] server={} ok={} output={:?}{}",
            tool, server, !tool_result.is_error, tool_result.text, err_tail
        );
        let result_into_target = match result_into {
            Some(s) if !s.is_empty() => s,
            _ => {
                if debug {
                    eprintln!("[mcp:debug] no result_into on adapter — skipping cascade");
                }
                return;
            }
        };
        let output_str = if tool_result.text.is_empty() {
            tool_result.structured.clone()
        } else {
            tool_result.text.clone()
        };
        self.cascade_tool_outcome(
            "mcp",
            result_into_target,
            invocation_id,
            tool,
            output_str,
            0,
            !tool_result.is_error,
            upstream_type,
            upstream_id,
            debug,
        );
    }
}
