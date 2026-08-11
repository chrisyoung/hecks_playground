//! file_io — the :read and :write leaves : run_read (offset/limit paging
//! via window_lines, truncated output) and run_write (create dirs as
//! needed, sentinel + contentless-write guards). Child cask of
//! claude_tool_dispatcher, sibling of glob.rs / edit_tool.rs.
//!
//! Cask extracted VERBATIM from claude_tool_dispatcher/mod.rs
//! (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/claude_tool_dispatcher/file_io.rs —
//!  kernel-floor tool leaves, relocated verbatim from mod.rs blanket.]

use super::{attr, err, truncate, ClaudeToolResult};
use std::collections::HashMap;
use std::path::Path;

// ── :read — read file_path, return contents (truncated) ──

/// Narrow file content to a 1-based line window `[offset, offset+limit)`.
/// `offset` defaults to line 1 ; `limit` defaults to the whole remainder.
/// Mirrors the underlying Read tool's offset/limit semantics so callers can
/// page large files instead of slurping from the top. When neither is given
/// the content is returned verbatim (backward-compatible whole-file read).
fn window_lines(content: &str, offset: Option<usize>, limit: Option<usize>) -> String {
    if offset.is_none() && limit.is_none() {
        return content.to_string();
    }
    let lines: Vec<&str> = content.lines().collect();
    let start = offset.unwrap_or(1).max(1) - 1; // 1-based → 0-based
    if start >= lines.len() {
        return String::new();
    }
    let end = match limit {
        Some(n) => start.saturating_add(n).min(lines.len()),
        None => lines.len(),
    };
    lines[start..end].join("\n")
}

pub(super) fn run_read(attrs: &HashMap<String, String>) -> ClaudeToolResult {
    let path = match attr(attrs, "file_path") {
        Some(p) => p.to_string(),
        None => return err("read", "missing required attr: file_path"),
    };
    let offset = attr(attrs, "offset").and_then(|s| s.trim().parse::<usize>().ok());
    let limit = attr(attrs, "limit").and_then(|s| s.trim().parse::<usize>().ok());
    match std::fs::read_to_string(&path) {
        Ok(c) => ClaudeToolResult {
            tool: "read".into(),
            ok: true,
            output: truncate(window_lines(&c, offset, limit)),
            exit_code: 0,
            error: None,
        },
        Err(e) => err("read", &format!("read {}: {}", path, e)),
    }
}

// ── :write — write content to file_path, creating dirs as needed ──

pub(super) fn run_write(attrs: &HashMap<String, String>) -> ClaudeToolResult {
    // Use the sentinel-aware `attr()` helper, NOT a raw `attrs.get()` : an unset
    // list-shaped file_path arrives as the rendered empty-list sentinel
    // "[0 items]". Without this guard `std::fs::write` happily creates a junk
    // file literally NAMED "[0 items]" (it kept reappearing on boot whenever a
    // cascade fired FileTool.Write/Update with no path). read + edit already
    // guard this ; write was the one creator that didn't. Fail loud instead.
    let path = match attr(attrs, "file_path") {
        Some(p) => p.to_string(),
        None => return err("write", "missing required attr: file_path"),
    };
    // A legit FileTool.Write/Update ALWAYS carries `content` (its whole
    // payload). A content-LESS :write is malformed and DESTRUCTIVE : it only
    // arises when a spurious cascade re-fires the write binding off accumulated
    // FileTool state, which persists `file_path` but NOT `content` (an
    // event-only field). The path is then a STALE real file and the body is
    // empty, so `fs::write` would TRUNCATE whatever was last read. Refuse loud,
    // exactly like the missing-file_path guard above, so a phantom write can
    // never empty a file. An INTENTIONAL empty write passes `content: ""` (the
    // key is PRESENT -> Some("")), so this only rejects the absent-content case.
    let content = match attrs.get("content") {
        Some(c) => c.clone(),
        None => return err("write", "refusing content-less write (would truncate); pass content (use \"\" to empty intentionally)"),
    };
    if let Some(parent) = Path::new(&path).parent() {
        if !parent.as_os_str().is_empty() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                return err("write", &format!("mkdir {}: {}", parent.display(), e));
            }
        }
    }
    match std::fs::write(&path, &content) {
        Ok(_) => ClaudeToolResult {
            tool: "write".into(),
            ok: true,
            output: format!("wrote {} bytes to {}", content.len(), path),
            exit_code: 0,
            error: None,
        },
        Err(e) => err("write", &format!("write {}: {}", path, e)),
    }
}
