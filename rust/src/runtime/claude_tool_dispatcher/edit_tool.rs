//! edit_tool — the :edit leaf : open file_path, replace old_string →
//! new_string, write back. Exact match first, then the tolerant
//! indentation-normalised line match (indent_len, locate_replace,
//! MatchErr) ; every failure mode names the path AND what went wrong
//! (the i559 diagnostics rule). Child cask of claude_tool_dispatcher,
//! sibling of glob.rs.
//!
//! Cask extracted VERBATIM from claude_tool_dispatcher/mod.rs
//! (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/claude_tool_dispatcher/edit_tool.rs
//!  — kernel-floor tool leaf, relocated verbatim from mod.rs blanket.]

use super::{err, ClaudeToolResult};
use std::collections::HashMap;

// ── :edit — open file_path, replace old_string → new_string, write ──
//
// Diagnostics rule of thumb : every failure mode names the path AND
// what specifically went wrong. The original implementation returned
// "old_string not found" with no context ; the i559 sidequest taught
// us that silent failures here cascade into broken Tools.Edit
// dispatches with `ok=false exit=1 output=""` and no clue why. So :
//
//   * missing path / old_string  → explicit "missing required attr" ;
//   * file-doesn't-exist          → "file not found: {path}" ;
//   * old_string not in file      → snippet of the file's first 200 chars ;
//   * old_string matches > 1 time → name the count, tell the caller how to fix ;
//   * write failure               → preserve the underlying io::Error.
//
// HECKS_DEBUG_CLAUDE_TOOL=1 prints the received old_string length +
// first 80 chars to stderr so the operator can see what arrived at
// the kernel hook (mangled newlines or escaped quotes from a shell
// arg parser would show up here).

pub(super) fn run_edit(attrs: &HashMap<String, String>) -> ClaudeToolResult {
    let path = match attrs.get("file_path") {
        Some(p) if !p.is_empty() && p != "[0 items]" => p.clone(),
        _ => return err("edit", "missing required attr: file_path"),
    };
    let old = match attrs.get("old_string") {
        Some(s) if !s.is_empty() && s != "[0 items]" => s.clone(),
        _ => return err("edit", "missing required attr: old_string"),
    };
    let new = attrs.get("new_string").cloned().unwrap_or_default();
    // The empty-list sentinel leaks through when the attribute is
    // unset on aggregate state ; treat it as "not provided" so
    // new_string defaults to empty rather than literally "[0 items]".
    let new = if new == "[0 items]" { String::new() } else { new };
    let replace_all = attrs.get("replace_all").map(|s| s == "true").unwrap_or(false);

    if std::env::var("HECKS_DEBUG_CLAUDE_TOOL").is_ok() {
        let preview: String = old.chars().take(80).collect();
        eprintln!(
            "[claude_tool:edit:debug] path={} old_len={} old_preview={:?}",
            path, old.len(), preview
        );
    }

    let contents = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return err("edit", &format!("file not found: {}", path));
        }
        Err(e) => return err("edit", &format!("read {}: {}", path, e)),
    };

    // Locate old_string and produce the edited contents. Exact (byte-for-byte)
    // match first ; if old_string isn't found exactly, fall back to a
    // whitespace-tolerant line match that compares lines by their TRIMMED
    // content and re-indents new_string to the file's actual indentation. This
    // rescues the recurring toil where old_string carried the wrong leading
    // whitespace (authored from memory, not the file's current bytes).
    let new_contents = match locate_replace(&contents, &old, &new, replace_all) {
        Ok(c) => c,
        Err(MatchErr::NotFound) => {
            // A peek of the file's head helps diagnose "did the value get
            // mangled in transit?" — a frequent culprit for multi-line strings.
            let head: String = contents.chars().take(200).collect();
            return err("edit", &format!(
                "old_string not found in {} ; first 200 chars of file: {:?}",
                path, head));
        }
        Err(MatchErr::Ambiguous(count)) => {
            return err("edit", &format!(
                "old_string appears {} times in {} — make it unique with more context, or pass replace_all=true",
                count, path));
        }
    };

    match std::fs::write(&path, &new_contents) {
        Ok(_) => ClaudeToolResult {
            tool: "edit".into(),
            ok: true,
            output: format!("edited {}", path),
            exit_code: 0,
            error: None,
        },
        Err(e) => err("edit", &format!("write {}: {}", path, e)),
    }
}

/// Outcome of locating an Edit's old_string in the file.
enum MatchErr {
    NotFound,
    Ambiguous(usize),
}

/// Count of leading ASCII spaces/tabs on a line (its indentation).
fn indent_len(line: &str) -> usize {
    line.chars().take_while(|c| *c == ' ' || *c == '\t').count()
}

/// Replace `old` with `new` in `contents`. EXACT match first (byte-for-byte,
/// unambiguous) ; on no exact match, a whitespace-TOLERANT fallback compares
/// lines by their trimmed content and re-indents `new` to the file's actual
/// indentation — rescuing the common case where `old` was authored with the
/// wrong leading whitespace. The tolerant path requires a UNIQUE trimmed-line
/// match (unless replace_all) so it can't silently edit the wrong region.
fn locate_replace(contents: &str, old: &str, new: &str, replace_all: bool) -> Result<String, MatchErr> {
    // Exact path — unchanged behavior for byte-exact old_strings.
    let exact = contents.matches(old).count();
    if exact == 1 || (replace_all && exact >= 1) {
        return Ok(if replace_all { contents.replace(old, new) } else { contents.replacen(old, new, 1) });
    }
    if exact > 1 {
        return Err(MatchErr::Ambiguous(exact));
    }

    // Tolerant fallback — match a contiguous run of lines by trimmed content.
    let old_body = old.strip_suffix('\n').unwrap_or(old);
    let new_body = new.strip_suffix('\n').unwrap_or(new);
    let old_lines: Vec<&str> = old_body.split('\n').collect();
    let n = old_lines.len();
    if n == 0 { return Err(MatchErr::NotFound); }
    let old_trim: Vec<&str> = old_lines.iter().map(|l| l.trim()).collect();

    let file_lines: Vec<&str> = contents.split('\n').collect();
    if file_lines.len() < n { return Err(MatchErr::NotFound); }

    let starts: Vec<usize> = (0..=file_lines.len() - n)
        .filter(|&i| (0..n).all(|j| file_lines[i + j].trim() == old_trim[j]))
        .collect();
    match starts.len() {
        0 => return Err(MatchErr::NotFound),
        1 => {}
        c => if !replace_all { return Err(MatchErr::Ambiguous(c)); }
    }

    // Splice replacements from LAST match to first so earlier indices stay
    // valid. Each new line is shifted by the delta between the file's actual
    // indent and old_string's first-line indent ; blank lines stay empty.
    let mut out: Vec<String> = file_lines.iter().map(|s| s.to_string()).collect();
    for &i in starts.iter().rev() {
        let delta = indent_len(file_lines[i]) as isize - indent_len(old_lines[0]) as isize;
        let reindented: Vec<String> = new_body.split('\n').map(|l| {
            if l.trim().is_empty() { return String::new(); }
            let cur = indent_len(l);
            let target = (cur as isize + delta).max(0) as usize;
            format!("{}{}", " ".repeat(target), &l[cur..])
        }).collect();
        out.splice(i..i + n, reindented);
    }
    Ok(out.join("\n"))
}

