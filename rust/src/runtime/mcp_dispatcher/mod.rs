//! [antibody-exempt: rust/src/runtime/mcp_dispatcher/mod.rs —
//!  kernel-floor handler for the `:mcp` hecksagon adapter family
//!  (i593). Implements the `invoke_mcp_tool` behavior_kind declared
//!  in framework/behavior_kinds/invoke_mcp_tool.hecksagon : one
//!  native primitive that opens a stdio MCP session against a named
//!  server, calls a named tool with a JSON arguments object, and
//!  captures the response. Sibling kernel-floor port to
//!  claude_tool_dispatcher / web_tool_dispatcher ; same exemption
//!  pattern, retires when the framework-wide kernel-hook registry
//!  replaces hard-coded handler tables.]
//!
//! McpDispatcher — kernel hook for the :mcp adapter family
//!
//! When a bluebook command dispatches and the matching hecksagon
//! carries an `adapter :mcp` row, the runtime locates the binding,
//! reads its `server`, `tool`, and `args` fields, and calls
//! `dispatch_via_registry` below. The dispatcher :
//!
//!   1. Resolves :server to a spawn command (v1 : :storehouse → node
//!      tooling/storehouse-mcp/src/server.mjs).
//!   2. Substitutes `{attr}` placeholders in :args from the dispatched
//!      command's attribute map.
//!   3. Spawns the server, does the JSON-RPC initialize → notifications/
//!      initialized → tools/call → close dance over stdio.
//!   4. Parses the tools/call response and returns a KernelResult with
//!      the tool's text content + isError flag.
//!
//! Pattern mirrors claude_tool_dispatcher : take the adapter's declared
//! fields + the dispatched command's attrs, execute, return a
//! structured result the Runtime can chain back via the adapter's
//! `result_into` target.
//!
//! ── Scope ──
//!
//! v1 is one-shot per call : each dispatch spawns its own MCP server
//! subprocess and tears it down on completion. Acceptable for the
//! SessionStart case (one call per session) and post-tool-use
//! (one call per tool dispatch). High-rate use cases (statusline
//! polls, every-tick policies) will want a long-lived session ; that
//! is a v2 path with the same field surface, different transport
//! state, filed as a follow-on when it bites.

mod registry_surface;
pub use registry_surface::{dispatch_via_registry, substitute_value};
mod session;
use session::McpSession;

use std::path::PathBuf;


use std::time::Duration;


// ─────────────────────────────────────────────────────────────────────
//  Server registry
// ─────────────────────────────────────────────────────────────────────

/// True when `server` names a LOCALLY-SPAWNABLE MCP server. The only such
/// server today is `:storehouse` (the local stdio Node server). World-
/// declared servers (e.g. `:gmail`, i610) are resolved by
/// `Runtime::resolve_mcp_adapters` from the `.world` file's `mcp` block
/// BEFORE this gate — they carry a `token_env` and a harness-owned
/// transport, so they never reach `resolve_server_spawn`. This predicate
/// remains the fallback gate for servers that are neither world-declared
/// nor spawnable. Trims a leading `:` so both `:storehouse` and
/// `storehouse` answer truthy ; matches `resolve_server_spawn`.
pub fn server_is_registered(server: &str) -> bool {
    let name = server.trim_start_matches(':');
    matches!(name, "storehouse")
}

/// Resolve the :server field to a (program, args) spawn pair.
///
/// v1 supports a single value : :storehouse → the local stdio server
/// at tooling/storehouse-mcp/src/server.mjs (relative to the hecks
/// repo root, resolved via the storehouse binary's path).
///
/// Returns `None` if the server name is unknown. Returns `Some(_)`
/// even when the path cannot be resolved — the dispatcher surfaces a
/// readable error on spawn failure rather than failing here.
fn resolve_server_spawn(server: &str) -> Option<(String, Vec<String>)> {
    // Trim a leading `:` so both `:storehouse` and `storehouse` work
    // — hecksagon parsers vary on whether the colon comes through.
    let name = server.trim_start_matches(':');
    match name {
        "storehouse" => {
            let server_path = locate_storehouse_mcp_server();
            Some(("node".to_string(), vec![server_path]))
        }
        _ => None,
    }
}

/// Walk from the current storehouse binary's path up the parent chain
/// looking for `tooling/storehouse-mcp/src/server.mjs`. Mirrors the
/// path-discovery pattern in run_boot/daemons.rs::resolve_body_dir.
/// Returns an empty string if not found ; spawn will fail loudly and
/// the dispatcher will report the error in its KernelResult.
fn locate_storehouse_mcp_server() -> String {
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return String::new(),
    };
    // The storehouse binary typically lives at rust/target/release/storehouse
    // or rust/target/debug/storehouse. Walk up to find the rust/ dir,
    // then sibling tooling/storehouse-mcp/src/server.mjs.
    let mut cur: PathBuf = match exe.parent() {
        Some(p) => p.to_path_buf(),
        None => return String::new(),
    };
    for _ in 0..6 {
        let candidate = cur.join("tooling/storehouse-mcp/src/server.mjs");
        if candidate.is_file() {
            return candidate.to_string_lossy().to_string();
        }
        if !cur.pop() {
            break;
        }
    }
    String::new()
}

// ─────────────────────────────────────────────────────────────────────
//  The one tool call
// ─────────────────────────────────────────────────────────────────────

/// What `dispatch` produced — the parsed text content from a tools/call
/// response, plus the structured payload and an error marker.
#[derive(Debug, Clone, Default)]
pub struct McpToolResult {
    /// Concatenated `text` content blocks from the MCP response.
    pub text: String,
    /// True when the server set isError=true OR transport failed.
    pub is_error: bool,
    /// The raw `structuredContent` from the response, JSON-encoded so
    /// downstream policies can re-parse without losing types.
    pub structured: String,
    /// Transport-level error message ; empty on success.
    pub error: String,
}

/// Dispatch one MCP tool call.
///
/// Spawns the named server, does the initialize / initialized / tools/call
/// / close dance, returns the captured response.
pub fn dispatch(
    server: &str,
    tool: &str,
    args: &serde_json::Value,
) -> McpToolResult {
    let mut out = McpToolResult::default();

    let (program, prog_args) = match resolve_server_spawn(server) {
        Some(p) => p,
        None => {
            out.is_error = true;
            out.error = format!("mcp : unknown server '{}' (v1 supports :storehouse only)", server);
            return out;
        }
    };

    let mut session = match McpSession::spawn(&program, &prog_args, server) {
        Ok(s) => s,
        Err(e) => {
            out.is_error = true;
            out.error = e;
            return out;
        }
    };

    // initialize — the MCP handshake. Send minimum-viable client info.
    let init_id = match session.send_request(
        "initialize",
        serde_json::json!({
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": { "name": "storehouse-mcp-client", "version": "0.1.0" },
        }),
    ) {
        Ok(id) => id,
        Err(e) => {
            out.is_error = true;
            out.error = e;
            return out;
        }
    };

    // Wait for initialize result with a soft timeout (server should
    // respond within seconds). We don't enforce timeout via thread —
    // BufRead::read_line blocks ; spawn-level failures already returned
    // above. If the server hangs, the dispatcher hangs ; v1 trade-off.
    let _init_result = match session.await_response(init_id) {
        Ok(v) => v,
        Err(e) => {
            out.is_error = true;
            out.error = format!("initialize : {}", e);
            return out;
        }
    };

    if let Err(e) = session.send_notification("notifications/initialized", serde_json::json!({})) {
        out.is_error = true;
        out.error = format!("notifications/initialized : {}", e);
        return out;
    }

    let call_id = match session.send_request(
        "tools/call",
        serde_json::json!({
            "name": tool,
            "arguments": args,
        }),
    ) {
        Ok(id) => id,
        Err(e) => {
            out.is_error = true;
            out.error = format!("tools/call : {}", e);
            return out;
        }
    };

    let call_result = match session.await_response(call_id) {
        Ok(v) => v,
        Err(e) => {
            out.is_error = true;
            out.error = format!("tools/call : {}", e);
            return out;
        }
    };

    // Extract content[].text — most tools return one text block, but
    // we concatenate to be safe.
    if let Some(content_array) = call_result.get("content").and_then(|v| v.as_array()) {
        let mut joined = String::new();
        for item in content_array {
            if item.get("type").and_then(|v| v.as_str()) == Some("text") {
                if let Some(t) = item.get("text").and_then(|v| v.as_str()) {
                    if !joined.is_empty() { joined.push('\n'); }
                    joined.push_str(t);
                }
            }
        }
        out.text = joined;
    }
    if call_result.get("isError").and_then(|v| v.as_bool()) == Some(true) {
        out.is_error = true;
    }
    if let Some(sc) = call_result.get("structuredContent") {
        out.structured = serde_json::to_string(sc).unwrap_or_default();
    }

    // Drop the session — Drop impl waits the child.
    let _ = session;
    let _ = Duration::from_millis(0); // marker : we intentionally don't enforce wait timeout
    out
}

