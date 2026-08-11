//! Rich dispatch-detail emitter — i697
//!
//! [antibody-exempt: rust/src/runtime/dispatch_detail.rs — rich storehouse
//!  log emitter, impl of the StorehouseEntry contract at
//!  framework/session/storehouse_log.bluebook. Sibling of
//!  rust/src/runtime/storehouse_log.rs (kernel-floor). Owns the
//!  thread-local per-dispatch event collector + the pretty-JSON,
//!  ANSI-coloured block one object per top-level dispatch. Retires
//!  alongside storehouse_log's exemption when the framework-wide
//!  kernel-hook registry absorbs the logging surface.]
//!
//! Where `storehouse_log` emits terse one-line records for interleaved
//! tracing, this module emits ONE rich, pretty-printed (2-space indent)
//! JSON block per top-level `Runtime::dispatch`, blank-line separated so
//! `tail -f` reads cleanly. It is the runtime side of the StorehouseEntry
//! bluebook : invocation_id, command, summary, args, outcome,
//! result_state, elapsed_ms, source_tag, metadata, and the full per-event
//! timeline (dispatch / event / cascade / adapter / policy / attr /
//! error / fallback).
//!
//! Lifecycle :
//!   1. `Runtime::dispatch` opens a `DispatchScope` as its first line.
//!      The scope reads `STOREHOUSE_SOURCE_TAG` (fallback HECKS_DAEMON →
//!      process-manager, else operator) and starts the wall-clock.
//!   2. The terse surfaces (`storehouse_log::dispatch_entry`,
//!      `event_emitted`, `cascade_step`, `policy_reaction`,
//!      `attribute_trace`) call `record_event` — which appends an
//!      EventTrace to the thread-local IFF a scope is open. Cascades go
//!      through `command_dispatch::dispatch_cascade`, NOT
//!      `Runtime::dispatch`, so there is exactly one open scope per
//!      top-level dispatch and the events form one clean tree.
//!   3. On scope drop (covers the `?` error path too) the collected
//!      timeline + the outcome/result fields the scope was fed are
//!      serialised into one rich block and emitted.
//!
//! ANSI + routing : the rich block is ANSI-coloured by kind and written
//! to the FILE sink ONLY (see `storehouse_log::emit_dual`) — never to the
//! dispatching process's stdout, which is reserved for the CLI's
//! contractual result line (the MCP wrapper + golden tests + piped
//! scripts parse stdout ; a pretty JSON block there contaminates them).
//! The "live stream" the block feeds is `storehouse follow` — a separate
//! tail process that reads the file and renders the colours natively.
//! `strip_ansi` (public) is the stripper for grep/parse-clean reads.

use std::cell::RefCell;

pub use super::dispatch_scope::DispatchScope;

// ── ANSI palette ────────────────────────────────────────────────────

pub const RESET: &str = "\x1b[0m";
pub const CYAN: &str = "\x1b[36m";
pub const GREEN: &str = "\x1b[32m";
pub const RED: &str = "\x1b[31m";
pub const BRIGHT_RED: &str = "\x1b[91m";
pub const MAGENTA: &str = "\x1b[35m";
pub const YELLOW: &str = "\x1b[33m";
pub const DIM: &str = "\x1b[2m";

/// Colour a string by event kind. dispatch=cyan, event/cascade-ok=green,
/// cascade-fail=red, adapter=magenta, error=bright-red, fallback=yellow,
/// policy/attr=dim. `ok` disambiguates cascade/adapter steps.
pub fn colour_for(kind: &str, ok: bool) -> &'static str {
    match kind {
        "dispatch" => CYAN,
        "event" => GREEN,
        "cascade" => if ok { GREEN } else { RED },
        "adapter" => if ok { MAGENTA } else { RED },
        "error" => BRIGHT_RED,
        "fallback" => YELLOW,
        "policy" | "attr" => DIM,
        _ => RESET,
    }
}

/// Strip ANSI SGR escape sequences (`\x1b[...m`) from a string so the
/// persistent file copy stays plain. Hand-rolled to avoid a dependency.
pub fn strip_ansi(s: &str) -> String {
    // Walk by `char`, never by byte. ANSI SGR markers are all ASCII so
    // the escape scan is byte-equivalent, but the body text carries
    // multi-byte UTF-8 (the `·` separator, accents in French summaries).
    // Pushing `byte as char` would split U+00B7 (C2 B7) into two chars
    // — the mojibake that poisoned the terse render. Iterating chars
    // keeps every codepoint whole.
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            chars.next(); // consume '['
            // Skip until the final byte of the CSI sequence (a letter).
            while let Some(&next) = chars.peek() {
                chars.next();
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

// ── EventTrace + thread-local collector ─────────────────────────────

/// One step in a dispatch's execution timeline — mirrors the EventTrace
/// VO in the bluebook (kind + verb + ok).
#[derive(Clone, Debug)]
pub struct EventTrace {
    pub kind: String,
    pub verb: String,
    pub ok: bool,
}

thread_local! {
    pub(super) static COLLECTOR: RefCell<Option<Vec<EventTrace>>> = const { RefCell::new(None) };
    /// Holds the event timeline from the most-recently-completed dispatch
    /// scope. Written by `DispatchScope::drop` immediately after the
    /// thread-local collector is drained. Callers (e.g. the warm serve
    /// path) call `take_last_events()` after `Runtime::dispatch` returns to
    /// retrieve the events for inclusion in the wire reply. A subsequent
    /// dispatch overwrites it ; `take_last_events` clears it on read.
    pub(super) static LAST_EVENTS: RefCell<Vec<EventTrace>> = const { RefCell::new(Vec::new()) };
}

/// True when a `DispatchScope` is currently open on this thread.
pub fn is_in_scope() -> bool {
    COLLECTOR.with(|c| c.borrow().is_some())
}

/// Append one EventTrace to the open scope, if any. Called by the terse
/// `storehouse_log` surfaces so the rich block captures the same timeline
/// they print. No-op when no scope is open (detached threads, ad-hoc
/// lines) so those surfaces stay terse-only.
pub fn record_event(kind: &str, verb: &str, ok: bool) {
    COLLECTOR.with(|c| {
        if let Some(v) = c.borrow_mut().as_mut() {
            v.push(EventTrace { kind: kind.to_string(), verb: verb.to_string(), ok });
        }
    });
}

/// Return and clear the event timeline from the most-recently-completed
/// dispatch scope. Called by the warm serve path immediately after
/// `Runtime::dispatch` returns so it can include the events in the RESULT
/// envelope. Returns an empty Vec when no dispatch has run yet (or after
/// a second call drains the same scope). Idempotent after drain.
pub fn take_last_events() -> Vec<EventTrace> {
    LAST_EVENTS.with(|s| std::mem::take(&mut *s.borrow_mut()))
}

// ── DispatchScope : RAII guard, emits the rich block on drop ────────

/// Resolve the source_tag from the dispatch context. Honors an explicit
/// `STOREHOUSE_SOURCE_TAG` env var ; else `HECKS_DAEMON=1` →
/// `process-manager` ; else `operator` (manual CLI + MCP default).
pub(super) fn resolve_source_tag() -> String {
    if let Ok(v) = std::env::var("STOREHOUSE_SOURCE_TAG") {
        if !v.is_empty() {
            return v;
        }
    }
    if std::env::var("HECKS_DAEMON").ok().as_deref() == Some("1") {
        return "process-manager".to_string();
    }
    "operator".to_string()
}

/// Parse a dispatched FQN into its `[Domain Aggregate Command]` parts.
/// `Domain::Aggregate.Command` → `("Domain", "Aggregate", "Command")`.
/// A domain-less `Aggregate.Command` → `("", "Aggregate", "Command")`.
/// Anything that doesn't split cleanly falls back to the whole string in
/// the command slot so the header never lies about a malformed FQN.
pub fn parse_fqn(fqn: &str) -> (String, String, String) {
    // Variable-depth Realm[::Subrealm…]::Domain::Aggregate.command — for display
    // we show the Domain (second-to-last :: segment) + Aggregate (last) + command.
    let (head, cmd) = match fqn.rsplit_once('.') {
        Some((h, c)) => (h, c.to_string()),
        None => (fqn, String::new()),
    };
    let segs: Vec<&str> = head.split("::").collect();
    let aggregate = segs.last().copied().unwrap_or("").to_string();
    let domain = if segs.len() >= 2 {
        segs[segs.len() - 2].to_string()
    } else {
        String::new()
    };
    (domain, aggregate, cmd)
}

/// Build the scannable bracketed header line for a dispatched FQN — EVERY
/// `::` namespace segment of the canonical address becomes its own word
/// (realm + subrealms + domain + aggregate + command), so the header is
/// realm-explicit and matches run_follow's dac_label. The command word is
/// bright-cyan, the rest dim. A bare / command-less FQN renders only the
/// parts it has, never an empty slot.
pub fn header_line(fqn: &str) -> String {
    let (head, command) = match fqn.rsplit_once('.') {
        Some((h, c)) => (h, c.to_string()),
        None => (fqn, String::new()),
    };
    let mut parts: Vec<String> = head
        .split("::")
        .filter(|w| !w.is_empty())
        .map(|w| format!("{}{}{}", DIM, w, RESET))
        .collect();
    if !command.is_empty() {
        parts.push(format!("{}{}{}", CYAN, command, RESET));
    }
    format!("{}[{}{}{}]{}", DIM, RESET, parts.join(" "), DIM, RESET)
}
