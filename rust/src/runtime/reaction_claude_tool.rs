//! reaction_claude_tool — the `:claude_tool` adapter family of Phase-3
//! reaction delivery : sugar (resolve_claude_tool_adapters) over the
//! `Primitive::ClaudeTool.Invoke` primitive (shell / edit / read / write /
//! grep / glob), the primitive-shape resolver, and the invoke engine
//! (run_claude_tool_invoke) routing through the shared cascade_tool_outcome
//! tail in reaction.rs. Inherent `impl Runtime` methods in a child module,
//! same contract as reaction.rs.
//!
//! Cask extracted VERBATIM from runtime/reaction.rs (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/reaction_claude_tool.rs — hand-written
//!  runtime kernel-floor, relocated verbatim from reaction.rs blanket.]

use super::*;
use std::collections::HashMap;

impl Runtime {
    /// `:claude_tool` adapter resolver — the HECKSAGON-FIRST native-tool edge
    /// (shell / edit / read / write / grep / glob), SUGAR over the
    /// `Primitive::ClaudeTool.Invoke` primitive exactly as its siblings. The
    /// hecksagon DECLARES the edge (`adapter :claude_tool, command:, tool:,
    /// result_into:`) ; the sugar's COMPOSITION step builds the tool attrs
    /// (upstream state fields + dispatch-attr overlay — i559 : Edit's
    /// old_string/new_string are event-only payloads that exist ONLY in the
    /// dispatch attrs, so the overlay wins) ; the one primitive RUNS the tool.
    /// Replaces the bespoke `resolve_claude_tool_adapters` resolver retired
    /// from runtime/mod.rs (shrink-mod phase A).
    pub(super) fn resolve_claude_tool_adapters(
        &mut self,
        result: &CommandResult,
        command_name: &str,
        dispatch_attrs: &HashMap<String, Value>,
    ) {
        if self.hecksagons.is_empty() {
            return;
        }
        let debug = std::env::var("HECKS_DEBUG_CLAUDE_TOOL").is_ok();
        let bare_command = command_name.rsplit('.').next().unwrap_or(command_name);
        let target = format!("{}.{}", result.aggregate_type, bare_command);
        if debug {
            eprintln!(
                "[claude_tool:debug] resolve cmd={} target={} hecksagons={}",
                command_name,
                target,
                self.hecksagons.len()
            );
        }
        let matched: Vec<(String, Option<String>)> = self
            .matched_io_bindings("claude_tool", &target, true, &["tool", "result_into"])
            .into_iter()
            .filter_map(|mut v| v[0].take().map(|t| (t, v[1].take())))
            .collect();
        if debug {
            eprintln!(
                "[claude_tool:debug] matched {} adapter(s) for target={}",
                matched.len(),
                target
            );
        }
        if matched.is_empty() {
            return;
        }
        // Composition : upstream state, dispatch attrs OVERLAID (i559 —
        // event-only payloads like Edit's old_string exist only in the
        // dispatch attrs ; user-supplied values win over any state echo).
        let mut attrs = self.state_attrs(&result.aggregate_type, &result.aggregate_id);
        for (k, v) in dispatch_attrs {
            attrs.insert(k.clone(), v.to_string());
        }
        let invocation_id = attrs
            .get("id")
            .cloned()
            .unwrap_or_else(|| result.aggregate_id.clone());
        for (tool, result_into) in &matched {
            self.run_claude_tool_invoke(
                tool,
                &attrs,
                result_into.as_deref(),
                &invocation_id,
                &result.aggregate_type,
                &result.aggregate_id,
                debug,
            );
        }
    }

    /// The generic `ClaudeTool.Invoke` primitive hook — sibling of the other
    /// resolve_primitive_* hooks. Fires when a `ClaudeTool.Invoke` command is
    /// dispatched : consults the PrimitiveRegistry (audit trail), reads tool /
    /// result_into / id from the dispatch attrs (the tool's own inputs ride
    /// the same attrs verbatim), and runs the one engine below. The bluebook
    /// IS the contract.
    pub(super) fn resolve_primitive_claude_tool(
        &mut self,
        result: &CommandResult,
        command_name: &str,
        dispatch_attrs: &HashMap<String, Value>,
    ) {
        let bare_command = command_name.rsplit('.').next().unwrap_or(command_name);
        if result.aggregate_type != "ClaudeTool" || bare_command != "Invoke" {
            return;
        }
        let registry_key = format!("{}.{}", result.aggregate_type, bare_command);
        self.log_primitive_registry_route(&registry_key);
        let Some(tool) = Self::required_primitive_attr("claude_tool", dispatch_attrs, "tool")
        else {
            return;
        };
        let result_into = dispatch_attrs.get("result_into").map(|v| v.to_string());
        let invocation_id = dispatch_attrs
            .get("id")
            .map(|v| v.to_string())
            .unwrap_or_else(|| result.aggregate_id.clone());
        let fn_attrs: HashMap<String, String> = dispatch_attrs
            .iter()
            .map(|(k, v)| (k.clone(), v.to_string()))
            .collect();
        let debug = std::env::var("HECKS_DEBUG_CLAUDE_TOOL").is_ok();
        self.run_claude_tool_invoke(
            &tool,
            &fn_attrs,
            result_into.as_deref(),
            &invocation_id,
            &result.aggregate_type,
            &result.aggregate_id,
            debug,
        );
    }

    /// The ONE claude_tool engine both paths share. Runs the kernel-floor
    /// `claude_tool_dispatcher` leaf — reused unchanged, the only imperative
    /// Rust on this path — prints the i622 stdout audit line, then chains the
    /// outcome through the shared `cascade_tool_outcome` tail. Missing
    /// result_into skips the cascade after the tool ran — exactly the retired
    /// resolver's contract.
    #[allow(clippy::too_many_arguments)]
    fn run_claude_tool_invoke(
        &mut self,
        tool: &str,
        attrs: &HashMap<String, String>,
        result_into: Option<&str>,
        invocation_id: &str,
        upstream_type: &str,
        upstream_id: &str,
        debug: bool,
    ) {
        let tool_result = claude_tool_dispatcher::dispatch(tool, attrs);
        let err_tail = match (&tool_result.ok, &tool_result.error) {
            (false, Some(msg)) => format!(" error={:?}", msg),
            _ => String::new(),
        };
        println!(
            "[{}] [claude_tool:{}] ok={} exit={} output={:?}{}",
            storehouse_log::now_iso8601(),
            tool, tool_result.ok, tool_result.exit_code, tool_result.output, err_tail
        );
        let result_into_target = match result_into {
            Some(s) if !s.is_empty() => s,
            _ => {
                if debug {
                    eprintln!("[claude_tool:debug] no result_into on adapter — skipping cascade");
                }
                return;
            }
        };
        self.cascade_tool_outcome(
            "claude_tool",
            result_into_target,
            invocation_id,
            &tool_result.tool,
            tool_result.output.clone(),
            tool_result.exit_code as i64,
            tool_result.ok,
            upstream_type,
            upstream_id,
            debug,
        );
    }
}
