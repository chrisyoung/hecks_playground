//! dispatch_scope — DispatchScope, the per-`Runtime::dispatch` rich-block
//! scope (i697) : opened as dispatch's first line, fed outcome/result
//! fields as they become known, emits the timeline block on Drop so the
//! `?` error path is still captured. The trace primitives (COLLECTOR,
//! EventTrace, record_event) stay in dispatch_detail.rs.
//!
//! Cask extracted VERBATIM from runtime/dispatch_detail.rs (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/dispatch_scope.rs — kernel-floor
//!  dispatch timeline, relocated verbatim from dispatch_detail.rs blanket.]

use super::dispatch_detail::{resolve_source_tag, COLLECTOR};
use super::dispatch_detail::LAST_EVENTS;
use super::storehouse_log;

/// Open per-`Runtime::dispatch` scope. Built as the first line of
/// `dispatch` ; fed the outcome/result fields as they become known ;
/// emits the rich block on drop (so the `?` error path is still
/// captured).
pub struct DispatchScope {
    invocation_id: String,
    command: String,
    args_json: String,
    source_tag: String,
    dispatched_at: String,
    started: std::time::Duration,
    outcome: String,
    result_state: String,
    pub(super) armed: bool,
}

impl DispatchScope {
    /// Begin a scope. Installs a fresh thread-local event collector and
    /// captures invocation_id, command, args, source_tag and the
    /// wall-clock start — everything the JSONL envelope needs.
    pub fn begin(invocation_id: &str, command: &str, args_json: String) -> Self {
        COLLECTOR.with(|c| *c.borrow_mut() = Some(Vec::new()));
        DispatchScope {
            invocation_id: invocation_id.to_string(),
            command: command.to_string(),
            args_json,
            source_tag: resolve_source_tag(),
            dispatched_at: storehouse_log::now_iso8601(),
            started: crate::clock::now_duration(),
            outcome: "ok".to_string(),
            result_state: String::new(),
            armed: true,
        }
    }

    /// Record the dispatch outcome (`ok` | `error`) + the result-state
    /// JSON (or the error message). Fed by `Runtime::dispatch` before the
    /// scope drops.
    pub fn finish(&mut self, outcome: &str, result_state: String) {
        self.outcome = outcome.to_string();
        self.result_state = result_state;
    }
}

impl Drop for DispatchScope {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let events = COLLECTOR.with(|c| c.borrow_mut().take()).unwrap_or_default();
        // Snapshot the events for the warm serve path BEFORE serialising
        // the file block. `take_last_events()` drains this after dispatch.
        LAST_EVENTS.with(|s| *s.borrow_mut() = events.clone());
        let elapsed_ms = crate::clock::now_duration()
            .saturating_sub(self.started)
            .as_millis() as u64;
        // JSONL event stream : one self-contained JSON object per emitted
        // event, one per line. The dispatch event carries `args` ; the
        // final `done` event carries outcome + elapsed_ms + event_count +
        // result. The watcher (`storehouse follow`) parses one object per
        // line and renders terse by default or raw with --json. One line =
        // one object, so there is never a multi-line block to re-assemble.
        let args: serde_json::Value =
            serde_json::from_str(&self.args_json).unwrap_or(serde_json::Value::Null);
        for (i, ev) in events.iter().enumerate() {
            let mut obj = serde_json::json!({
                "ts": self.dispatched_at,
                "invocation_id": self.invocation_id,
                "command": self.command,
                "kind": ev.kind,
                "verb": ev.verb,
                "ok": ev.ok,
                "source": self.source_tag,
            });
            if i == 0 && ev.kind == "dispatch" && !args.is_null() {
                obj["args"] = args.clone();
            }
            storehouse_log::emit_file(&obj.to_string());
        }
        let result: serde_json::Value = serde_json::from_str(&self.result_state)
            .unwrap_or_else(|_| serde_json::Value::String(self.result_state.clone()));
        let done = serde_json::json!({
            "ts": self.dispatched_at,
            "invocation_id": self.invocation_id,
            "command": self.command,
            "kind": "done",
            "outcome": self.outcome,
            "elapsed_ms": elapsed_ms,
            "event_count": events.len(),
            "result": result,
            "source": self.source_tag,
        });
        storehouse_log::emit_file(&done.to_string());
    }
}
