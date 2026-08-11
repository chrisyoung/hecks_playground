//! log_sink — the storehouse log's write path : the lazy Mutex'd FILE_SINK
//! (stdout-only after a failed open), size-based rotation (maybe_rotate +
//! the HECKS_LOG_ROTATE_* knobs), sink_write, emit / emit_stdout /
//! emit_file. Entry formatting (dispatch_entry & co) and the level gate
//! stay in storehouse_log.rs ; old public paths hold via re-exports.
//!
//! Cask extracted VERBATIM from runtime/storehouse_log.rs (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/log_sink.rs — kernel-floor log sink,
//!  relocated verbatim from storehouse_log.rs blanket.]

use super::storehouse_log::log_file_path;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::{Mutex, OnceLock};

// Lazy-initialized append-mode file handle wrapped in a Mutex so
// concurrent dispatches don't interleave lines. The OnceLock holds
// `None` if the open failed once and we've given up — stdout-only
// from there.
static FILE_SINK: OnceLock<Option<Mutex<std::fs::File>>> = OnceLock::new();

fn file_sink_init() -> Option<Mutex<std::fs::File>> {
    let path = log_file_path();
    if let Some(parent) = path.parent() {
        // Best-effort directory creation. If this fails the open will
        // fail next and we'll fall into the warn-and-skip branch.
        let _ = std::fs::create_dir_all(parent);
    }
    match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(f) => Some(Mutex::new(f)),
        Err(e) => {
            eprintln!(
                "[storehouse_log] warning : cannot open log file {} ({}) — stdout only",
                path.display(),
                e
            );
            None
        }
    }
}

// ── Rotation : bound the diagnostic log's growth ───────────────────
// storehouse.log is a best-effort diagnostic firehose that EVERY process (the
// dispatch door + every daemon) appends to as ONE shared file, so it grows
// without bound (it reached 1 GB+). Rotation caps it : when the live file
// exceeds HECKS_LOG_MAX_BYTES, the writer that notices renames it
// (storehouse.log -> .1 -> .2 … up to HECKS_LOG_KEEP, oldest dropped) and
// reopens a fresh file. MULTI-WRITER convergence : a sibling that still holds
// the now-renamed fd notices its fd is larger than the live path and reopens,
// so all writers re-converge on the fresh file (size-based, no inode —
// portable). Checked every ROTATE_CHECK_EVERY writes (amortized : no stat per
// line). Best-effort : a torn line in the rename window is fine for a
// diagnostic log. Config via env, deployment-level like heki's dir.
const ROTATE_CHECK_EVERY: u64 = 256;

fn rotate_max_bytes() -> u64 {
    std::env::var("HECKS_LOG_MAX_BYTES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(64 * 1024 * 1024) // 64 MiB
}

fn rotate_keep() -> u32 {
    std::env::var("HECKS_LOG_KEEP")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3)
}

static WRITES_SINCE_BOOT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Rotate (or reopen) the sink when the shared log file outgrew the cap or was
/// rotated by a sibling. Called under the sink Mutex, every ROTATE_CHECK_EVERY
/// writes. Size-based : if the live path exceeds the cap WE rotate ; else if
/// the held fd is larger than the live path, a sibling already rotated and we
/// reopen the live path.
pub(super) fn maybe_rotate(f: &mut std::fs::File, path: &std::path::Path, max_bytes: u64, keep: u32) {
    let live_len = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let our_len = f.metadata().map(|m| m.len()).unwrap_or(0);
    if live_len > max_bytes {
        let keep = keep.max(1);
        let p = path.to_string_lossy();
        let _ = std::fs::remove_file(format!("{}.{}", p, keep));
        for i in (1..keep).rev() {
            let _ = std::fs::rename(format!("{}.{}", p, i), format!("{}.{}", p, i + 1));
        }
        let _ = std::fs::rename(path, format!("{}.1", p));
        if let Ok(nf) = OpenOptions::new().create(true).append(true).open(path) {
            *f = nf;
        }
    } else if our_len > live_len {
        // A sibling rotated the file out from under us — reopen the live path.
        if let Ok(nf) = OpenOptions::new().create(true).append(true).open(path) {
            *f = nf;
        }
    }
}

/// Append one line to the file sink, rotating first when due. The single place
/// the rotate-check + write live ; `emit` (stdout + file) and `emit_file`
/// (file only) both funnel through here.
fn sink_write(line: &str) {
    if let Some(sink) = FILE_SINK.get_or_init(file_sink_init) {
        if let Ok(mut f) = sink.lock() {
            if WRITES_SINCE_BOOT.fetch_add(1, std::sync::atomic::Ordering::Relaxed).is_multiple_of(ROTATE_CHECK_EVERY)
            {
                maybe_rotate(&mut f, &log_file_path(), rotate_max_bytes(), rotate_keep());
            }
            // Errors here are swallowed deliberately — stdout already carried
            // the breadcrumb, and a per-write warning would spam stderr.
            let _ = writeln!(f, "{}", line);
        }
    }
}

/// Emit one line to both stdout and the append-mode file sink. All five
/// public surfaces funnel through here so the dual-sink contract lives
/// in exactly one place. `println!` goes first so a panicking file
/// write never costs the operator the stdout breadcrumb.
pub(super) fn emit(line: String) {
    println!("{}", line);
    sink_write(&line);
}

/// Emit a legacy per-event breadcrumb to stdout ONLY — never to the file
/// sink. The dispatch/event/cascade/policy/attr breadcrumbs are the
/// dispatching process's stdout contract : the MCP wrapper parses them
/// into its events[] timeline and the golden smoke asserts them. But in
/// the FILE they are pure redundancy — every top-level dispatch already
/// writes a rich i697 block (`emit_dual`) carrying the same event
/// timeline. Keeping breadcrumbs out of the file makes `storehouse
/// follow` a clean stream of rich blocks with no legacy-line noise
/// (Chris : "eliminate the legacy lines — bring them up to date"). The
/// follow-on (filed inbox) retires breadcrumbs from stdout too, once the
/// MCP wrapper reads events from the rich block instead of parsing them.
pub(super) fn emit_stdout(line: String) {
    println!("{}", line);
}

/// Append one plain line to the FILE sink only — the flat event stream
/// `storehouse follow` watches. stdout is untouched (the breadcrumbs +
/// the `{"ok":...}` result line own stdout for the MCP wrapper + golden
/// tests). The file is plain (no ANSI) ; the watcher applies colour.
pub fn emit_file(line: &str) {
    sink_write(line);
}
