//! event_shard — the append-only, one-JSON-line-per-event shard that backs
//! the out-of-process Event Log (the shards-plus-merge topology, Chris
//! 2026-06-20 ; see inbox/event-log-oop-precondition.md).
//!
//! WHY bespoke, not a per-process heki file : heki is whole-file
//! read-modify-write (see heki::write_raw). A per-process heki shard would
//! be single-writer (no clobber) but still re-serialize the whole file on
//! every append — which destroys byte-offset resume (the merge daemon would
//! have to re-read + decompress the entire shard every tick). A bespoke
//! append-only format gives TRUE offset resume : the merge daemon tails each
//! shard from where it left off. And because each shard has exactly ONE
//! writer (the owning process), there is no cross-process interleave — so the
//! >4KB O_APPEND-atomicity caveat that killed the multi-writer design does
//! > not apply here AT ALL ; a record may be any size.
//!
//! Each line is one self-contained JSON `ShardRecord`. The record carries
//! everything the merge needs to impose a global total order WITHOUT a later
//! format break : `shard` (origin), `seq` (per-process monotonic), `ts`
//! (wall-clock). The merge orders by (ts, shard, seq).
//!
//! SLICE 1 (this file) : format + writer + offset-reader, pure + tested.
//! Does NOT touch record_event_append (still gated off). Slice 2 is the
//! merge daemon + the N-process losslessness proof ; slice 3 rewires the
//! writer + flips the gate. Un-gating is the LAST act, never before merge.

use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

/// One immutable event in a process shard : the merge ENVELOPE (the total-order
/// key shard/seq/ts plus the dedup key event_id) and the EVENT itself, carried
/// OPAQUELY as the serialized EventSourcing::Event aggregate (its fields as a
/// JSON object). The shard line no longer hand-lists the Event's fields — the
/// AppendLog adapter serializes the Event AggregateState into `event`, so the
/// Event shape has ONE source (the bluebook), not a parallel struct that drifts.
/// The merge orders by (ts, shard, seq) and writes `event` (the true nested
/// shape — command{verb,inputs}, delta{field,value}, sequence{value}, …) to the
/// global Log, so a reader of the global store sees the real Event, not a
/// flattened copy.
#[derive(Debug, Clone, PartialEq)]
pub struct ShardRecord {
    // ── merge envelope : the total-order key is (ts, shard, seq) ; event_id dedups ──
    pub shard: String,
    pub seq: u64,
    pub ts: String,
    pub event_id: String,
    // ── the event : the Event aggregate's fields, serialized (no duplication) ──
    pub event: serde_json::Map<String, serde_json::Value>,
}

/// Serialize one record to its single-line JSON form (no trailing newline ;
/// the writer adds it). JSON escapes any embedded newline in field values,
/// so the one-record-per-line invariant always holds. Built by hand from a
/// serde_json::Map — the crate carries serde_json but not serde-derive, so
/// this mirrors record_event_append's manual-map style.
pub fn serialize_line(rec: &ShardRecord) -> String {
    use serde_json::Value;
    let mut m = serde_json::Map::new();
    m.insert("shard".into(), Value::String(rec.shard.clone()));
    m.insert("seq".into(), Value::Number(rec.seq.into()));
    m.insert("ts".into(), Value::String(rec.ts.clone()));
    m.insert("event_id".into(), Value::String(rec.event_id.clone()));
    m.insert("event".into(), Value::Object(rec.event.clone()));
    Value::Object(m).to_string()
}

/// Parse one line back to a record. Strict : a complete line that fails to
/// parse (bad JSON or a missing field) is a genuine corruption — single-writer
/// never tears a line — so the caller gets an Err rather than a silent drop.
pub fn parse_line(line: &str) -> Result<ShardRecord, String> {
    let v: serde_json::Value =
        serde_json::from_str(line).map_err(|e| format!("shard parse: {}", e))?;
    let s = |k: &str| -> Result<String, String> {
        v.get(k)
            .and_then(|x| x.as_str())
            .map(|x| x.to_string())
            .ok_or_else(|| format!("shard parse: missing/!str field '{}'", k))
    };
    let seq = v.get("seq")
        .and_then(|x| x.as_u64())
        .ok_or_else(|| "shard parse: missing/!u64 field 'seq'".to_string())?;
    let event = v.get("event")
        .and_then(|x| x.as_object())
        .cloned()
        .ok_or_else(|| "shard parse: missing/!object field 'event'".to_string())?;
    Ok(ShardRecord {
        shard: s("shard")?,
        seq,
        ts: s("ts")?,
        event_id: s("event_id")?,
        event,
    })
}

/// A process-private append-only shard. ONE writer per shard (the owning
/// process), so appends never interleave and the file is never rewritten.
pub struct ShardWriter {
    file: std::fs::File,
}

impl ShardWriter {
    /// Open (creating if absent) a shard for append. The path should be
    /// process-private (keyed by shard id) so this is the sole writer.
    pub fn open(path: &Path) -> std::io::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self { file })
    }

    /// Append one record as a single line and flush, so a crash right after
    /// the call cannot lose an acknowledged event.
    pub fn append(&mut self, rec: &ShardRecord) -> std::io::Result<()> {
        let line = serialize_line(rec);
        writeln!(self.file, "{}", line)?;
        self.file.flush()
    }
}

/// Read every COMPLETE record from `path` starting at byte `offset`, and
/// return them with the new offset (the byte position after the last
/// complete line consumed). A partial trailing line — a write in flight — is
/// NOT consumed ; it is left for the next read. `offset` is always on a line
/// boundary (a previous return value or 0), hence always a UTF-8 boundary.
pub fn read_from(path: &Path, offset: u64) -> std::io::Result<(Vec<ShardRecord>, u64)> {
    let mut f = match std::fs::File::open(path) {
        Ok(f) => f,
        // No shard yet : nothing to read, offset unchanged.
        Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((Vec::new(), offset)),
        Err(e) => return Err(e),
    };
    f.seek(SeekFrom::Start(offset))?;
    let mut buf = String::new();
    f.read_to_string(&mut buf)?;

    // Consume only up to the last newline ; anything after is a partial
    // line still being written.
    let consumed = match buf.rfind('\n') {
        Some(i) => i + 1,
        None => 0,
    };
    let mut records = Vec::new();
    for line in buf[..consumed].lines() {
        if line.trim().is_empty() {
            continue;
        }
        match parse_line(line) {
            Ok(r) => records.push(r),
            Err(e) => {
                return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, e));
            }
        }
    }
    Ok((records, offset + consumed as u64))
}

// ── Process-private singleton shard (the live write path) ──────────────
// One shard per process, opened once and appended to for the process's

pub use super::shard_sink::{append_record, append_to_process_shard, process_shard_id, reserve};
