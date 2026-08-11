//! StoreHouse stdout + file logger — i622
//!
//! [antibody-exempt: rust/src/runtime/storehouse_log.rs — kernel-floor
//!  sibling of rust/src/runtime/mod.rs. Owns the one-line stdout
//!  records that mod.rs's dispatch path emits ; lives in Rust because
//!  it touches stdout from the runtime hot path and reads the
//!  STOREHOUSE_LOG env var once at process start. Retires alongside
//!  runtime/mod.rs's existing kernel-floor exemption when the
//!  framework-wide kernel-hook registry absorbs the dispatch
//!  surface — at that point the logger moves to a bluebook-defined
//!  `:log` adapter family.]
//!
//! One-line stdout records for every dispatch, event, cascade step,
//! and policy reaction. Level gated by the `STOREHOUSE_LOG` env var
//! read once at process start (`init`/`level`).
//!
//! Levels :
//!   - `quiet`   — dispatch entry + policy reactions only
//!   - `normal`  — dispatch + event + cascade + policy   (default)
//!   - `verbose` — normal + per-attribute trace
//!
//! In addition to stdout, every emitted line ALSO appends to a stable
//! file path so a sibling process (`storehouse follow`) can tail and
//! filter the bus from another terminal. File path resolution :
//!
//!   1. `STOREHOUSE_LOG_FILE` env var, if set and non-empty.
//!   2. `<heki::resolve_info_dir()>/storehouse.log` — the same
//!      `miette-state/information` sibling the rest of the runtime
//!      writes to.
//!
//! If the file can't be opened (permission, missing dir), a single
//! stderr warning fires and stdout-only emission continues. The file
//! sink follows the exact same level gating as stdout.
//!
//! Usage :
//!   storehouse_log::dispatch_entry("Tools::ShellTool.Bash", "abc123", Some("run echo"));
//!   storehouse_log::event_emitted("ShellTool", "BashRan", "abc123");
//!   storehouse_log::cascade_step("Cascade.RecordResult", "abc123", true);
//!   storehouse_log::policy_reaction("OnBashRan", "ShellTool", "BashRan", "abc123", "Cascade.RecordResult");
//!   storehouse_log::attribute_trace("ShellTool", "abc123", "shell_command", "echo hi");
//!   storehouse_log::mcp_stderr_line("storehouse", "server listening on stdio");
//!
//! Lines are written via `println!` so the runtime's stdout is the
//! single audit stream. See `hecks_conception/inbox/i622.md` for the
//! design and `i613.md` for the envelope shape this record reflects.

pub use super::log_time::{interpolate_now_tokens, now_iso8601};
pub use super::log_sink::emit_file;
use super::log_sink::{emit, emit_stdout};
use std::sync::OnceLock;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LogLevel {
    Quiet,
    Normal,
    Verbose,
}

static LEVEL: OnceLock<LogLevel> = OnceLock::new();

/// Resolve and cache the log level from `STOREHOUSE_LOG`. Defaults to
/// `Normal` when unset or unrecognised. Idempotent : first call wins,
/// subsequent calls return the cached value (matching the i622 design
/// note that the level is read once at process start).
pub fn level() -> LogLevel {
    *LEVEL.get_or_init(|| match std::env::var("STOREHOUSE_LOG").ok().as_deref() {
        Some("quiet")   => LogLevel::Quiet,
        Some("verbose") => LogLevel::Verbose,
        _               => LogLevel::Normal,
    })
}

/// Test-only override. Resets the cached level. Not exposed to callers
/// outside the runtime — only `#[cfg(test)]` paths in this crate touch
/// it. Safe to call before any logging happens.
#[cfg(test)]
pub fn set_level_for_test(new_level: LogLevel) {
    // OnceLock has no public reset ; we read and ignore. Tests that
    // need to flip levels run with a single fixed env value per
    // process — the dedicated cargo-test integration spawns
    // subprocesses (see `rust/tests/storehouse_log_test.rs`).
    let _ = LEVEL.set(new_level);
}

/// Resolve the storehouse log file path. Honors `STOREHOUSE_LOG_FILE`
/// first, then falls back to `<heki::resolve_info_dir()>/storehouse.log`
/// so the file lands next to the rest of the runtime's heki records.
/// Public so `storehouse follow` can resolve the same path the writer
/// uses without duplicating the env-var contract.
pub fn log_file_path() -> std::path::PathBuf {
    if let Ok(v) = std::env::var("STOREHOUSE_LOG_FILE") {
        if !v.is_empty() {
            return std::path::PathBuf::from(v);
        }
    }
    crate::heki::resolve_info_dir().join("storehouse.log")
}

/// Surface 1 — dispatch entry. Printed when a top-level command enters
/// the runtime. `description` is optional (the human-readable label
/// from the command declaration, when known).
pub fn dispatch_entry(fqn: &str, id: &str, description: Option<&str>) {
    // i697 — feed the rich-block timeline (no-op outside a dispatch scope).
    crate::runtime::dispatch_detail::record_event("dispatch", fqn, true);
    if level() == LogLevel::Quiet || level() == LogLevel::Normal || level() == LogLevel::Verbose {
        let desc = description
            .map(|d| format!(" \"{}\"", d))
            .unwrap_or_default();
        emit_stdout(format!("[{}] dispatch {}#{}{}", now_iso8601(), fqn, id, desc));
    }
}

/// Surface 2 — event emission. Printed when a dispatch yields its
/// event. Suppressed at `Quiet`.
pub fn event_emitted(aggregate: &str, event_name: &str, id: &str) {
    crate::runtime::dispatch_detail::record_event(
        "event", &format!("{}.{}", aggregate, event_name), true);
    if level() == LogLevel::Normal || level() == LogLevel::Verbose {
        emit_stdout(format!("[{}] event {}.{}#{}", now_iso8601(), aggregate, event_name, id));
    }
}

/// Surface 3 — cascade chain step. Printed when a cascade dispatch
/// fires (claude_tool result_into, policy trigger, PM dispatch, etc.).
/// `ok` reflects the cascade outcome (`true` on success, `false` on
/// dispatch error). Suppressed at `Quiet`.
pub fn cascade_step(fqn: &str, id: &str, ok: bool) {
    crate::runtime::dispatch_detail::record_event("cascade", fqn, ok);
    if level() == LogLevel::Normal || level() == LogLevel::Verbose {
        emit_stdout(format!("[{}] cascade {}#{} ok={}", now_iso8601(), fqn, id, ok));
    }
}

/// Surface 4 — policy reaction. Printed when a policy fires in response
/// to an event and dispatches a follow-up command. Visible at every
/// level (policy chains are too important to silence even at `Quiet`).
pub fn policy_reaction(
    policy_name: &str,
    aggregate: &str,
    event_name: &str,
    id: &str,
    dispatched_command: &str,
) {
    crate::runtime::dispatch_detail::record_event(
        "policy", &format!("{} -> {}", policy_name, dispatched_command), true);
    if level() == LogLevel::Quiet || level() == LogLevel::Normal || level() == LogLevel::Verbose {
        emit_stdout(format!(
            "[{}] policy {} on {}.{}#{} -> {}",
            now_iso8601(), policy_name, aggregate, event_name, id, dispatched_command
        ));
    }
}

/// Surface 5 — per-attribute trace. Only printed at `Verbose`.
/// One line per attribute set into aggregate state during a dispatch.
pub fn attribute_trace(aggregate: &str, id: &str, attr: &str, value: &str) {
    crate::runtime::dispatch_detail::record_event(
        "attr", &format!("{}.{}", aggregate, attr), true);
    if level() == LogLevel::Verbose {
        // Truncate noisy values so the verbose trace stays one-line.
        let v_trimmed = if value.len() > 80 {
            format!("{}…", &value[..80])
        } else {
            value.to_string()
        };
        emit_stdout(format!(
            "[{}] attr {}#{} {}={}",
            now_iso8601(), aggregate, id, attr, v_trimmed
        ));
    }
}

/// MCP child stderr — one log line per line captured from a `:mcp`
/// adapter's child process. Visible at every level (these are
/// substrate-level diagnostics that operators expect to see when an
/// MCP server misbehaves). `server` is the resolved server name
/// (e.g. `storehouse`) ; `line` is the raw stderr line, trimmed.
pub fn mcp_stderr_line(server: &str, line: &str) {
    emit(format!("[{}] [mcp:{}] {}", now_iso8601(), server, line));
}
