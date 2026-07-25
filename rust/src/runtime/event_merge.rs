//! event_merge — the merge half of the out-of-process Event Log (slice 2).
//!
//! The shards-plus-merge topology : N dispatch processes each single-write
//! their own append-only shard (event_shard.rs) ; ONE merge daemon tails
//! every shard and folds them into the global ordered `event.heki`. Because
//! the merge daemon is the SOLE writer of the global log, the global store
//! may use ordinary whole-file heki — the clobber that started all this only
//! ever bit CONCURRENT writers, and here there is exactly one.
//!
//! A merge PASS : for each shard, read every record written since that
//! shard's checkpoint offset ; collect across all shards ; sort the batch by
//! the total-order key (ts, shard, seq) ; return it with the advanced
//! checkpoint. The daemon then appends the sorted batch to the global log
//! (assigning the authoritative monotonic global sequence at append) and
//! persists the checkpoint. Per-shard byte offsets make every pass resume
//! exactly where the last left off — no event read twice, none skipped.
//!
//! This module is the pure, runtime-free core (file IO + sort + checkpoint),
//! so the losslessness proof — 30 concurrent writers, 30/30 events survive,
//! the case the old whole-file Event repo dropped to 13/30 — is a direct
//! unit test. The `storehouse merge` daemon wrapper + the record_event_append
//! rewiring + the gate flip are the following slices.
//!
//! [antibody-exempt: rust/src/runtime/event_merge.rs — kernel-floor persistence
//!  IO. The pure, runtime-free shard-merge + reclaim core (file IO + sort +
//!  checkpoint + GC of dead, fully-folded shards), sibling of heki.rs /
//!  event_shard.rs. The CONCEPT is conceived in the EventSourcing bluebook
//!  (Consolidation.Consolidate — fold then reclaim) ; this is its hand-written
//!  runtime realization. A shard is a persistence ARTIFACT below the aggregate
//!  (like a .heki file), so modeling it as a domain aggregate would be the
//!  PersistenceIsAggregateOnly anti-pattern — the substrate the language bottoms
//!  out on, not the language. Same kernel-floor contract as projection_fold.rs.]

use super::event_shard::{read_from, ShardRecord};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Per-shard byte-offset high-water marks : shard id -> next unread offset.
pub type Checkpoint = HashMap<String, u64>;

/// One shard to merge : its stable id (the checkpoint key) and its path.
pub struct Shard {
    pub id: String,
    pub path: PathBuf,
}

/// Discover every `*.shard` file in `dir` as a Shard (id = file stem). Empty
/// (not an error) when the dir is absent — the daemon may start before any
/// process has written a shard.
pub fn discover_shards(dir: &Path) -> std::io::Result<Vec<Shard>> {
    let mut shards = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(shards),
        Err(e) => return Err(e),
    };
    for entry in entries {
        let path = entry?.path();
        if path.extension().and_then(|x| x.to_str()) == Some("shard") {
            if let Some(stem) = path.file_stem().and_then(|x| x.to_str()) {
                shards.push(Shard { id: stem.to_string(), path });
            }
        }
    }
    // Stable discovery order so a pass over identical state is deterministic.
    shards.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(shards)
}

/// Run one merge pass over `shards`, advancing `checkpoint` in place. Returns
/// every record written since the last pass, sorted by the total-order key
/// (ts, shard, seq). The caller appends this batch to the global log and
/// persists the (now advanced) checkpoint.
pub fn merge_pass(shards: &[Shard], checkpoint: &mut Checkpoint) -> std::io::Result<Vec<ShardRecord>> {
    let mut batch = Vec::new();
    for shard in shards {
        let from = checkpoint.get(&shard.id).copied().unwrap_or(0);
        let (records, new_off) = read_from(&shard.path, from)?;
        batch.extend(records);
        checkpoint.insert(shard.id.clone(), new_off);
    }
    batch.sort_by(|a, b| {
        a.ts.cmp(&b.ts)
            .then(a.shard.cmp(&b.shard))
            .then(a.seq.cmp(&b.seq))
    });
    Ok(batch)
}

/// Load a checkpoint map from a plain JSON file (single-writer : the merge
/// daemon). Missing file -> empty checkpoint (a fresh merge).
pub fn load_checkpoint(path: &Path) -> std::io::Result<Checkpoint> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Checkpoint::new()),
        Err(e) => return Err(e),
    };
    let v: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    let mut cp = Checkpoint::new();
    if let Some(obj) = v.as_object() {
        for (k, val) in obj {
            if let Some(n) = val.as_u64() {
                cp.insert(k.clone(), n);
            }
        }
    }
    Ok(cp)
}

/// Append a merged batch to the global Event Log. The global Log is an
/// APPEND-ONLY JSONL file (event_log.rs), NOT a whole-file heki store : heki
/// is for snapshots, the LOG is append-only (Chris, 2026-06-21 ; the shards
/// already abandoned heki for the same reason). So each merge appends the new
/// batch — O(batch) — instead of decompress-insert-recompress the whole log
/// (O(whole-log)) every 5s, and the file is directly `tail -f`-able.
///
/// The merge is the SOLE writer, so a plain O_APPEND is lossless. The global
/// `sequence` is the persisted monotonic counter event_log owns (no whole-file
/// read to compute store.len()). Dedup rides the per-shard CHECKPOINT (consumed
/// bytes are never re-read — see merge_pass), with the fold's idempotence to
/// duplicate deltas as the crash-replay backstop ; write_batch_to_global is a
/// pure append. Returns the count appended.
pub fn write_batch_to_global(global_path: &str, batch: &[ShardRecord]) -> Result<usize, String> {
    super::event_log::append_records(global_path, batch)
        .map_err(|e| format!("event-log append {}: {}", global_path, e))
}

/// Persist a checkpoint map as JSON. Written by the sole merge daemon, so a
/// plain whole-file write is safe.
pub fn save_checkpoint(path: &Path, cp: &Checkpoint) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut m = serde_json::Map::new();
    for (k, off) in cp {
        m.insert(k.clone(), serde_json::Value::Number((*off).into()));
    }
    std::fs::write(path, serde_json::Value::Object(m).to_string())
}

/// Parse the owning pid from a shard id of the form `p{pid}-{nonce}`
/// (event_shard::process_shard_id). None for any id that does not match —
/// such a shard is never reclaimed (conservative).
pub fn owner_pid(shard_id: &str) -> Option<u32> {
    shard_id.strip_prefix('p')?.split('-').next()?.parse().ok()
}

/// Reclaim per-process shard files the merge has already fully folded into the
/// global Log AND whose owning process is dead. Returns the count deleted and
/// prunes each reclaimed shard's checkpoint entry (so the checkpoint does not
/// grow without bound — it tracked one entry per process that ever wrote).
/// The caller persists the pruned checkpoint.
///
/// Two guards, both REQUIRED, make this lossless and crash-safe :
///   1. FULLY FOLDED : checkpoint offset >= on-disk size, so every byte is
///      already in event.heki. A shard with an unconsumed tail (offset < size)
///      is left for the next merge to fold first, THEN reclaim.
///   2. OWNER DEAD : a LIVE process holds its shard's file handle open
///      (event_shard PROCESS_SHARD) ; unlinking it would send that process's
///      future appends to a ghost inode, silently lost. So liveness is checked
///      via the injected `is_alive` predicate. pid-reuse only makes this MORE
///      conservative (a reused pid reads as alive -> skip). The predicate is
///      injected (not a syscall here) so this core stays pure and unit-testable
///      — the caller supplies the real process-table lookup, and MUST pass a
///      predicate that errs toward `alive` when liveness cannot be determined.
pub fn reclaim_consumed_shards(
    shards: &[Shard],
    checkpoint: &mut Checkpoint,
    is_alive: &dyn Fn(u32) -> bool,
) -> usize {
    let mut reclaimed = 0usize;
    for shard in shards {
        let off = checkpoint.get(&shard.id).copied().unwrap_or(0);
        let size = match std::fs::metadata(&shard.path) {
            Ok(m) => m.len(),
            Err(_) => continue, // gone already, or unstattable : skip
        };
        if off < size {
            continue; // unconsumed tail : the next merge folds it first
        }
        // Unparseable id, or a live owner : never reclaim.
        match owner_pid(&shard.id) {
            Some(pid) if !is_alive(pid)
                && std::fs::remove_file(&shard.path).is_ok() => {
                    checkpoint.remove(&shard.id);
                    reclaimed += 1;
                }
            _ => {}
        }
    }
    reclaimed
}
