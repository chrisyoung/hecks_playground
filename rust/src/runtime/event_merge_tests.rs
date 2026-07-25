//! event_merge_tests — the shard-merge suite : shard discovery, ordered
//! merge pass, checkpoint round-trip, global-append batch, consumed-shard
//! reclaim.
//!
//! Cask extracted VERBATIM from runtime/event_merge.rs (cask-runtime) ;
//! body dedented one level out of the old inline mod.
//!
//! [antibody-exempt: rust/src/runtime/event_merge_tests.rs — kernel-floor
//!  merge tests, relocated verbatim from event_merge.rs blanket.]

use super::event_merge::*;
use crate::runtime::event_shard::{ShardRecord, ShardWriter};
use std::path::PathBuf;

fn rec(shard: &str, seq: u64, ts: &str) -> ShardRecord {
    let mut event = serde_json::Map::new();
    event.insert("event_id".into(), serde_json::Value::String(format!("{}-{}", shard, seq)));
    event.insert("aggregate_name".into(), serde_json::Value::String("Order".into()));
    event.insert("aggregate_id".into(), serde_json::Value::String(format!("o-{}", seq)));
    event.insert("value".into(), serde_json::Value::String("pending".into()));
    ShardRecord {
        shard: shard.into(),
        seq,
        ts: ts.into(),
        event_id: format!("{}-{}", shard, seq),
        event,
    }
}

fn tmp(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("event_merge_{}_{}", std::process::id(), name))
}

#[test]
fn merge_pass_sorts_by_total_order_key() {
    let dir = tmp("sort");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // Two shards, interleaved timestamps ; the merge must order by ts.
    let mut a = ShardWriter::open(&dir.join("a.shard")).unwrap();
    let mut b = ShardWriter::open(&dir.join("b.shard")).unwrap();
    a.append(&rec("a", 1, "2026-06-20T10:00:01Z")).unwrap();
    a.append(&rec("a", 2, "2026-06-20T10:00:03Z")).unwrap();
    b.append(&rec("b", 1, "2026-06-20T10:00:02Z")).unwrap();
    let shards = discover_shards(&dir).unwrap();
    let mut cp = Checkpoint::new();
    let batch = merge_pass(&shards, &mut cp).unwrap();
    let order: Vec<&str> = batch.iter().map(|r| r.event_id.as_str()).collect();
    assert_eq!(order, vec!["a-1", "b-1", "a-2"]); // by ts: 01, 02, 03
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn merge_pass_resumes_no_replay_no_skip() {
    let dir = tmp("resume");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut a = ShardWriter::open(&dir.join("a.shard")).unwrap();
    a.append(&rec("a", 1, "t1")).unwrap();
    let mut cp = Checkpoint::new();
    let first = merge_pass(&discover_shards(&dir).unwrap(), &mut cp).unwrap();
    assert_eq!(first.len(), 1);
    // More appends ; the next pass sees ONLY the new ones (no replay).
    a.append(&rec("a", 2, "t2")).unwrap();
    a.append(&rec("a", 3, "t3")).unwrap();
    let second = merge_pass(&discover_shards(&dir).unwrap(), &mut cp).unwrap();
    assert_eq!(second.iter().map(|r| r.seq).collect::<Vec<_>>(), vec![2, 3]);
    // A pass with nothing new yields nothing (no skip, no replay).
    let third = merge_pass(&discover_shards(&dir).unwrap(), &mut cp).unwrap();
    assert!(third.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn checkpoint_round_trips() {
    let path = tmp("cp.json");
    let _ = std::fs::remove_file(&path);
    let mut cp = Checkpoint::new();
    cp.insert("a".into(), 128);
    cp.insert("b".into(), 4096);
    save_checkpoint(&path, &cp).unwrap();
    let loaded = load_checkpoint(&path).unwrap();
    assert_eq!(loaded.get("a"), Some(&128));
    assert_eq!(loaded.get("b"), Some(&4096));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn replayed_batch_does_not_duplicate_in_global_log() {
    // CRASH-SAFETY. The daemon writes the global log BEFORE saving the
    // checkpoint ; if it crashes in between, the SAME batch is merged
    // again next boot. Idempotency by event_id must make that replay a
    // no-op — each event in the global log exactly once, never twice.
    let dir = tmp("crash");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut w = ShardWriter::open(&dir.join("a.shard")).unwrap();
    w.append(&rec("a", 1, "t1")).unwrap();
    w.append(&rec("a", 2, "t2")).unwrap();
    let mut cp = Checkpoint::new();
    let global = dir.join("event.log");
    let gp = global.to_str().unwrap();

    // First pass : merge_pass returns the 2 records ; the append lands them.
    let batch = merge_pass(&discover_shards(&dir).unwrap(), &mut cp).unwrap();
    let n1 = write_batch_to_global(gp, &batch).unwrap();
    assert_eq!(n1, 2, "first pass appends both events");
    // Idempotence MOVED : the global Log is now append-only JSONL (not a
    // dedup'd heki map), so write_batch_to_global is a pure append. Crash-
    // safety rides the CHECKPOINT — once it has advanced, a second merge_pass
    // returns NOTHING, so each event lands in the global Log exactly once.
    let batch2 = merge_pass(&discover_shards(&dir).unwrap(), &mut cp).unwrap();
    assert!(batch2.is_empty(), "checkpoint prevents re-reading consumed shard bytes");
    let states = crate::runtime::event_log::load_states(gp);
    assert_eq!(states.len(), 2, "global Log holds each event exactly once");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn thirty_concurrent_writers_lose_nothing() {
    // THE PROOF. 30 processes-worth of concurrent writers, each the SOLE
    // writer of its OWN shard, 50 events each. The merge must recover all
    // 30*50 = 1500 — the case the old whole-file Event repo dropped to
    // 13/30. Single-writer-per-shard is what buys losslessness.
    let dir = tmp("concurrent");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    const WRITERS: u64 = 30;
    const PER: u64 = 50;
    let handles: Vec<_> = (0..WRITERS).map(|w| {
        let dir = dir.clone();
        std::thread::spawn(move || {
            let id = format!("p{:02}", w);
            let mut writer = ShardWriter::open(&dir.join(format!("{}.shard", id))).unwrap();
            for s in 1..=PER {
                let ts = format!("2026-06-20T10:{:02}:{:02}Z", w % 60, s % 60);
                writer.append(&rec(&id, s, &ts)).unwrap();
            }
        })
    }).collect();
    for h in handles {
        h.join().unwrap();
    }
    let mut cp = Checkpoint::new();
    let batch = merge_pass(&discover_shards(&dir).unwrap(), &mut cp).unwrap();
    assert_eq!(batch.len() as u64, WRITERS * PER, "every concurrent event must survive");
    // And they are unique (no torn/duplicated records).
    let mut ids: Vec<&str> = batch.iter().map(|r| r.event_id.as_str()).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len() as u64, WRITERS * PER, "no duplicates, no corruption");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reclaim_deletes_only_dead_and_fully_folded_shards() {
    // The two guards in one table. Three shards in the p{pid}-{nonce} form
    // so owner_pid parses :
    //   p111 : dead owner, fully folded   -> RECLAIM
    //   p222 : dead owner, unconsumed tail -> keep (merge folds it first)
    //   p333 : ALIVE owner, fully folded   -> keep (open handle ; ghost-inode)
    let dir = tmp("reclaim");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut a = ShardWriter::open(&dir.join("p111-1.shard")).unwrap();
    let mut b = ShardWriter::open(&dir.join("p222-2.shard")).unwrap();
    let mut c = ShardWriter::open(&dir.join("p333-3.shard")).unwrap();
    a.append(&rec("p111-1", 1, "t1")).unwrap();
    b.append(&rec("p222-2", 1, "t1")).unwrap();
    c.append(&rec("p333-3", 1, "t1")).unwrap();
    let shards = discover_shards(&dir).unwrap();
    // Fold everything to EOF, then write MORE to b so its offset < size.
    let mut cp = Checkpoint::new();
    let _ = merge_pass(&shards, &mut cp).unwrap();
    b.append(&rec("p222-2", 2, "t2")).unwrap();

    let is_alive = |pid: u32| pid == 333; // only p333 lives
    let n = reclaim_consumed_shards(&shards, &mut cp, &is_alive);

    assert_eq!(n, 1, "only the dead, fully-folded shard is reclaimed");
    assert!(!dir.join("p111-1.shard").exists(), "dead + folded -> deleted");
    assert!(dir.join("p222-2.shard").exists(), "unconsumed tail -> kept");
    assert!(dir.join("p333-3.shard").exists(), "alive owner -> kept");
    assert!(!cp.contains_key("p111-1"), "reclaimed shard's checkpoint entry pruned");
    assert!(cp.contains_key("p222-2"), "kept shard's checkpoint entry remains");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn owner_pid_parses_shard_id_form() {
    assert_eq!(owner_pid("p12345-1781999116413003000"), Some(12345));
    assert_eq!(owner_pid("p7-0"), Some(7));
    assert_eq!(owner_pid("nonsense"), None); // never reclaimed
    assert_eq!(owner_pid("p-1"), None);
}
