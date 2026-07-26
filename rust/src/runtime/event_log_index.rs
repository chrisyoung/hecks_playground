//! event_log_index — the offset index for the append-only Event Log (Phase 3)
//!
//! Maps the Log's primary partition key (`aggregate_name`) to the byte offsets
//! of its events in `event.log`, so `Event.Replay(name)` SEEKS to its events
//! instead of streaming the whole Log.
//!
//! SAFE BY CONSTRUCTION — no false negative is possible :
//!   * The index is maintained ONLY by the single writer (the consolidate
//!     merge, via event_log::append_records). Readers never write it, so there
//!     is no reader-vs-reader race.
//!   * `event.log.idx.len` is the COMMIT MARKER, written LAST (after the log
//!     append and after the index entries). It records the log byte-length the
//!     index is complete through.
//!   * A reader uses the index ONLY when `idx.len == current log length` — i.e.
//!     the index is provably complete. After a crash (log appended, marker not
//!     yet advanced) the lengths differ, so readers fall back to the full
//!     streaming scan (always correct) until the next append self-heals.
//!   * The writer self-heals : if the marker doesn't match the pre-append log
//!     length (a prior crash, or a deleted index file), it REBUILDS the whole
//!     index from the log before appending the new entries.
//!
//! Format : `event.log.idx2` is one `<byte_offset>\t<aggregate_name>\t<sequence>`
//! line per event, in log order ; `event.log.idx2.len` holds a single integer
//! (the commit marker). The name column backs `Replay`'s Eq-by-name seek ; the
//! sequence column (ascending, since log order == sequence order) backs the
//! sequence-range seek (`complete_offsets_in_seq_range`, binary-searchable).
//! The `.idx2` filename supersedes the legacy 2-column `.idx` (see idx_path).

use std::io::{BufRead, Write};

/// The Log field the index partitions on — the Replay key.
pub const INDEXED_FIELD: &str = "aggregate_name";

// `.idx2` (NOT `.idx`) : the index format gained a 3rd column (sequence) for the
// sequence-range seek. Bumping the filename is the migration — the legacy 2-col
// `.idx` is simply never read, so it can't be mis-parsed ; the new `.idx2` is
// absent on first run, so record_appends rebuilds it from the log (the existing
// self-heal path) and stamps its marker last. Safe-by-construction, no data op.
fn idx_path(global: &str) -> String {
    format!("{}.idx2", global)
}

fn len_path(global: &str) -> String {
    format!("{}.idx2.len", global)
}

fn log_len(global: &str) -> u64 {
    std::fs::metadata(global).map(|m| m.len()).unwrap_or(0)
}

fn read_marker(global: &str) -> u64 {
    std::fs::read_to_string(len_path(global))
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

pub(super) fn write_marker(global: &str, n: u64) {
    let _ = std::fs::write(len_path(global), n.to_string());
}

/// The indexed field's value from a raw JSONL line, or None if absent / the
/// line isn't a JSON object (such a line simply isn't indexed — it can't match
/// an aggregate_name query anyway).
fn indexed_value(line: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    v.as_object()?
        .get(INDEXED_FIELD)?
        .as_str()
        .map(|s| s.to_string())
}

/// The sequence value from a raw JSONL line — the nested `{"sequence":{"value":N}}`
/// the merge stamps. None if absent / unparseable (a line with no sequence gets 0
/// in the rebuilt index ; it can't meaningfully match a sequence-range query).
fn indexed_sequence(line: &str) -> Option<u64> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    v.as_object()?.get("sequence")?.as_object()?.get("value")?.as_u64()
}

/// Rebuild the entire index from `log[0..up_to]`, then stamp the marker. Used by
/// the writer when the index is out of sync (crash gap, missing file, first
/// build). O(log) but only on the cold path ; steady-state append is O(batch).
fn rebuild(global: &str, up_to: u64) {
    let mut idx_buf = String::new();
    if let Ok(file) = std::fs::File::open(global) {
        let mut reader = std::io::BufReader::new(file);
        let mut offset: u64 = 0;
        let mut line = String::new();
        while offset < up_to {
            line.clear();
            let n = match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(n) => n,
                Err(_) => break,
            };
            if let Some(name) = indexed_value(line.trim_end()) {
                let seq = indexed_sequence(line.trim_end()).unwrap_or(0);
                idx_buf.push_str(&format!("{}\t{}\t{}\n", offset, name, seq));
            }
            offset += n as u64;
        }
    }
    let _ = std::fs::write(idx_path(global), idx_buf);
    write_marker(global, up_to);
}

/// Single-writer hook : record an append. `l0` is the log byte-length BEFORE
/// this append ; `entries` are the `(start_offset, aggregate_name)` of the new
/// lines ; `new_total_len` is the log byte-length AFTER. Must be called AFTER
/// the log bytes are durably written. Self-heals (rebuilds) when the index
/// isn't already complete through `l0`, then appends and advances the marker.
pub fn record_appends(global: &str, l0: u64, entries: &[(u64, String, u64)], new_total_len: u64) {
    let in_sync = std::path::Path::new(&idx_path(global)).exists() && read_marker(global) == l0;
    if !in_sync {
        rebuild(global, l0);
    }
    let mut buf = String::new();
    for (off, name, seq) in entries {
        buf.push_str(&format!("{}\t{}\t{}\n", off, name, seq));
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(idx_path(global))
    {
        let _ = f.write_all(buf.as_bytes());
    }
    // The COMMIT MARKER, written last : everything above is now durable.
    write_marker(global, new_total_len);
}

/// Reader : the byte offsets of `name`'s events — but ONLY when the index is
/// provably complete (`marker == log length`). `None` means "index not usable,
/// full-scan instead" : incomplete after a crash, missing, or malformed. Never
/// returns a partial set that could drop a real match.
pub fn complete_offsets_for(global: &str, name: &str) -> Option<Vec<u64>> {
    if read_marker(global) != log_len(global) {
        return None;
    }
    let text = std::fs::read_to_string(idx_path(global)).ok()?;
    let mut offsets = Vec::new();
    for line in text.lines() {
        // 3-col : <offset>\t<name>\t<sequence>. Name is the 2nd field ; the
        // sequence (3rd) is unused for the name-Eq seek.
        let mut parts = line.splitn(3, '\t');
        let off: u64 = parts.next()?.parse().ok()?;
        if parts.next().unwrap_or("") == name {
            offsets.push(off);
        }
    }
    Some(offsets)
}

/// Reader : byte offsets of every event whose sequence is in `[lo, hi]`
/// (inclusive ; `None` = open on that side), in log order — but ONLY when the
/// index is provably complete (`marker == log length`), else `None` (stream
/// fallback, always correct). The sequence column is ascending (append order ==
/// sequence order), so this binary-searches to the first in-range row and walks
/// forward. Gaps are fine ; a SUPERSET is acceptable (the oracle re-filters).
/// Bounds are already collapsed from Gt/Gte/Lt/Lte to inclusive lo/hi.
pub fn complete_offsets_in_seq_range(
    global: &str,
    lo: Option<u64>,
    hi: Option<u64>,
) -> Option<Vec<u64>> {
    if read_marker(global) != log_len(global) {
        return None;
    }
    let text = std::fs::read_to_string(idx_path(global)).ok()?;
    // (offset, sequence) per line ; lines without a parseable sequence are
    // dropped (they can't match a sequence range). Order is preserved, so the
    // surviving rows stay sequence-ascending.
    let rows: Vec<(u64, u64)> = text
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\t');
            let off: u64 = parts.next()?.parse().ok()?;
            let _name = parts.next()?;
            let seq: u64 = parts.next()?.parse().ok()?;
            Some((off, seq))
        })
        .collect();
    let start = match lo {
        Some(lo) => rows.partition_point(|(_, seq)| *seq < lo),
        None => 0,
    };
    let mut offsets = Vec::new();
    for (off, seq) in &rows[start..] {
        if let Some(hi) = hi {
            if *seq > hi {
                break;
            }
        }
        offsets.push(*off);
    }
    Some(offsets)
}
