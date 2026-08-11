//! shard_sink — the per-process shard sink singleton : process_shard_id
//! (pid + boot nonce, q2 shard identity), the lazy Mutex'd open_sink,
//! reserve (seq allocation), append_record / append_to_process_shard. The
//! record codec + ShardWriter + read_from stay in event_shard.rs ; old
//! `event_shard::` paths hold via re-exports.
//!
//! Cask extracted VERBATIM from runtime/event_shard.rs (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/shard_sink.rs — kernel-floor shard
//!  sink, relocated verbatim from event_shard.rs blanket.]

use super::event_shard::{ShardRecord, ShardWriter};
use std::path::Path;

// life — the single-writer property that makes the append lossless at any
// size. Mirrors storehouse_log::FILE_SINK : a process-global OnceLock so
// record_event_append (called per dispatch, &mut self) writes without
// threading a ShardWriter through the Runtime struct.
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

static PROCESS_SHARD: OnceLock<Option<Mutex<ShardWriter>>> = OnceLock::new();
static SHARD_SEQ: AtomicU64 = AtomicU64::new(0);
static SHARD_ID: OnceLock<String> = OnceLock::new();

/// Stable per-process-BOOT shard id : pid + a boot nonce (nanos since epoch),
/// so a restart that reuses a pid never writes into the prior boot's shard
/// (shard-identity question q2). Computed once.
pub fn process_shard_id() -> &'static str {
    SHARD_ID.get_or_init(|| {
        let pid = std::process::id();
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        format!("p{}-{}", pid, nonce)
    })
}

/// Lazily open (once per process) the private shard sink rooted at `shard_dir`
/// as `<shard-id>.shard`. Returns the sink, or None if it could not be opened
/// (warned once — a failed Log write never crashes the dispatch). The single
/// place the open path lives ; shared by `reserve` + `append_to_process_shard`.
fn open_sink(shard_dir: &Path) -> Option<&'static Mutex<ShardWriter>> {
    PROCESS_SHARD
        .get_or_init(|| {
            let path = shard_dir.join(format!("{}.shard", process_shard_id()));
            match ShardWriter::open(&path) {
                Ok(w) => Some(Mutex::new(w)),
                Err(e) => {
                    eprintln!("[event_shard] cannot open process shard ({}) — events not logged", e);
                    None
                }
            }
        })
        .as_ref()
}

/// Reserve the next (shard_id, seq) for THIS process WITHOUT writing — the
/// AppendLog persistence adapter's first half of a split write. Event.Append is
/// `identified_by :event_id`, so the runtime must mint the globally-unique id
/// BEFORE dispatch (the command needs it to resolve identity + carry it through
/// the invariant), then the adapter's `save` persists the built record. The id
/// is `event_id = "{shard}-{seq}"` — (shard, per-process seq) is unique across
/// shards, exactly the merge's dedup key. Inits the sink under `shard_dir` on
/// first call ; None if it can't open.
pub fn reserve(shard_dir: &Path) -> Option<(String, u64)> {
    open_sink(shard_dir)?;
    let seq = SHARD_SEQ.fetch_add(1, Ordering::SeqCst) + 1;
    Some((process_shard_id().to_string(), seq))
}

/// Append an ALREADY-stamped record (shard/seq/event_id set, e.g. from
/// `reserve`) to this process's sink — no fetch_add, no re-stamp. Returns false
/// if the sink isn't open or the write fails (warned, never panics). The sink
/// must have been opened by a prior `reserve` / `append_to_process_shard`.
pub fn append_record(rec: &ShardRecord) -> bool {
    let sink = match PROCESS_SHARD.get() {
        Some(Some(m)) => m,
        _ => return false,
    };
    match sink.lock() {
        Ok(mut w) => match w.append(rec) {
            Ok(()) => true,
            Err(e) => {
                eprintln!("[event_shard] shard append failed: {}", e);
                false
            }
        },
        Err(_) => false,
    }
}

/// Append one event to THIS process's private shard. The convenience entry that
/// combines `reserve` (mint shard/seq, stamp event_id) with `append_record`
/// (write) ; the AppendLog adapter splits the two halves instead (reserve at
/// dispatch, write at save). Returns the assigned seq, or None if the shard
/// can't be opened/written.
pub fn append_to_process_shard(shard_dir: &Path, mut rec: ShardRecord) -> Option<u64> {
    let (shard, seq) = reserve(shard_dir)?;
    rec.shard = shard.clone();
    rec.seq = seq;
    // event_id is the merge's dedup key, globally unique across shards.
    rec.event_id = format!("{}-{}", shard, seq);
    if append_record(&rec) {
        Some(seq)
    } else {
        None
    }
}
