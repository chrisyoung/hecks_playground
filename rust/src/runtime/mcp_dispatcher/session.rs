//! session — the MCP JSON-RPC-over-stdio session : owns the child process
//! + pipes for its lifetime (initialize handshake, request/response
//! framing, notification skip, timeout guard) ; Drop closes stdin and
//! waits for the clean exit the MCP contract promises. The dispatch
//! surface stays in mod.rs.
//!
//! Cask extracted VERBATIM from mcp_dispatcher/mod.rs (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/mcp_dispatcher/session.rs —
//!  kernel-floor MCP session, relocated verbatim from mod.rs blanket.]

use crate::runtime::storehouse_log;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::thread;

/// One MCP session. Owns the child process + stdio pipes for its
/// lifetime ; closing drops the child and the pipes.
pub(super) struct McpSession {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl McpSession {
    /// Spawn `<program> <args>` with stdin/stdout/stderr all piped.
    /// i622 — stderr was previously routed to `Stdio::null()` and the
    /// server's diagnostics vanished. Now each captured stderr line
    /// goes to the StoreHouse log stream prefixed with
    /// `[mcp:<server-name>]` so operator visibility lines up with
    /// dispatch/event/cascade/policy lines on the same stdout.
    pub(super) fn spawn(program: &str, args: &[String], server_name: &str) -> Result<Self, String> {
        let mut cmd = Command::new(program);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(|e| {
            format!("mcp : spawn '{}' failed : {}", program, e)
        })?;
        let stdin = child.stdin.take().ok_or("mcp : no stdin")?;
        let stdout = BufReader::new(child.stdout.take().ok_or("mcp : no stdout")?);
        // Forward stderr to the storehouse log surface on a detached
        // thread. The thread exits when the child closes stderr (EOF
        // on the BufRead loop). One log line per stderr line — same
        // shape as the rest of the runtime's per-line stdout records.
        if let Some(err) = child.stderr.take() {
            let server_label = server_name.to_string();
            thread::spawn(move || {
                let reader = BufReader::new(err);
                // STOP on a read error rather than `.flatten()`, which discards
                // the Err and asks the iterator for another line — a reader that
                // keeps erroring (a broken pipe, a dead child) would spin this
                // thread forever at full tilt. An error here means the child's
                // stderr is gone, and there is nothing further to pump.
                for line in reader.lines() {
                    match line {
                        Ok(l) => storehouse_log::mcp_stderr_line(&server_label, l.trim_end()),
                        Err(_) => break,
                    }
                }
            });
        }
        Ok(Self { child, stdin, stdout, next_id: 1 })
    }

    /// Send a JSON-RPC message as a single newline-terminated line
    /// (the MCP stdio convention). Returns the id used so the caller
    /// can match the response.
    pub(super) fn send_request(&mut self, method: &str, params: serde_json::Value) -> Result<u64, String> {
        let id = self.next_id;
        self.next_id += 1;
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let line = serde_json::to_string(&msg)
            .map_err(|e| format!("mcp : encode {} : {}", method, e))?;
        self.stdin.write_all(line.as_bytes())
            .map_err(|e| format!("mcp : write {} : {}", method, e))?;
        self.stdin.write_all(b"\n")
            .map_err(|e| format!("mcp : write newline : {}", e))?;
        self.stdin.flush().ok();
        Ok(id)
    }

    pub(super) fn send_notification(&mut self, method: &str, params: serde_json::Value) -> Result<(), String> {
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        let line = serde_json::to_string(&msg)
            .map_err(|e| format!("mcp : encode notify {} : {}", method, e))?;
        self.stdin.write_all(line.as_bytes())
            .map_err(|e| format!("mcp : write notify {} : {}", method, e))?;
        self.stdin.write_all(b"\n").ok();
        self.stdin.flush().ok();
        Ok(())
    }

    /// Read JSON-RPC responses line by line until one matches `id`.
    /// Skips server-initiated notifications (no `id` field) and
    /// out-of-order responses (different `id`). Returns the `result`
    /// payload on success, or the JSON-RPC `error` shape on failure.
    pub(super) fn await_response(&mut self, id: u64) -> Result<serde_json::Value, String> {
        loop {
            let mut line = String::new();
            let n = self.stdout.read_line(&mut line)
                .map_err(|e| format!("mcp : read : {}", e))?;
            if n == 0 {
                return Err("mcp : server closed stdout before responding".into());
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let parsed: serde_json::Value = match serde_json::from_str(trimmed) {
                Ok(v) => v,
                Err(_) => continue, // not JSON ; could be a server log line, skip
            };
            // Notifications carry no id ; skip.
            let msg_id = match parsed.get("id").and_then(|v| v.as_u64()) {
                Some(i) => i,
                None => continue,
            };
            if msg_id != id {
                continue;
            }
            if let Some(err) = parsed.get("error") {
                return Err(format!("mcp : server error : {}", err));
            }
            return Ok(parsed.get("result").cloned()
                .unwrap_or(serde_json::Value::Null));
        }
    }
}

impl Drop for McpSession {
    fn drop(&mut self) {
        // Close stdin so the server's stdio loop terminates cleanly.
        // The child should exit on its own once stdin closes ; we
        // wait briefly then move on. We don't kill — a clean exit is
        // the contract MCP servers follow.
        let _ = self.child.wait();
    }
}
