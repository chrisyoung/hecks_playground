//! es_log_migration — the Log substrate migration + inspection tail :
//! migrate_event_log_to_jsonl (one-time heki → append-only JSONL move,
//! reversible via .premigration, refuses if event.log exists) and
//! unconsolidated_log_tail (the records newer than the last merge fold).
//! Inherent `impl Runtime` methods in a child module, same contract as
//! event_sourcing.rs.
//!
//! Cask extracted VERBATIM from runtime/event_sourcing.rs (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/es_log_migration.rs — kernel-floor
//!  runtime, relocated verbatim from event_sourcing.rs blanket.]

use super::*;

impl Runtime {
    /// One-time migration of the global Event Log from compressed whole-file
    /// heki (`event.heki`) to the append-only JSONL substrate (`event.log`).
    /// Reads the heki store, orders records by sequence, writes them as JSONL,
    /// seeds the `.seq` sidecar from the max sequence, and renames the old heki
    /// aside (`.premigration`, reversible). Returns (before, after) counts.
    /// Refuses if `event.log` already exists. Idempotent at the realm level :
    /// run once per realm, before the new append-only writer goes live.
    pub fn migrate_event_log_to_jsonl(&self) -> Result<(usize, usize), String> {
        let es_key = repo_key(Some("EventSourcing"), "Event");
        let store_dir = self
            .repositories
            .get(&es_key)
            .and_then(|r| r.heki_path())
            .ok_or("EventSourcing::Event not loaded (no store dir)")?;
        let old = crate::heki::path_for(&store_dir, "Event", Some("EventSourcing"));
        let new = event_log::global_path(&store_dir, Some("EventSourcing"));
        if std::path::Path::new(&new).exists() {
            return Err(format!("{} already exists — already migrated", new));
        }
        let store = crate::heki::read(&old).map_err(|e| format!("read {}: {}", old, e))?;
        let before = store.len();
        let seq_of = |r: &crate::heki::Record| -> i64 {
            r.get("sequence")
                .and_then(|v| v.as_object())
                .and_then(|m| m.get("value"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0)
        };
        let mut records: Vec<&crate::heki::Record> = store.values().collect();
        records.sort_by_key(|r| seq_of(r));
        if let Some(parent) = std::path::Path::new(&new).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut buf = String::new();
        let mut max_seq = 0i64;
        for r in &records {
            max_seq = max_seq.max(seq_of(r));
            // heki::Record is a HashMap ; serialize it as a JSON object line.
            let obj: serde_json::Map<String, serde_json::Value> =
                r.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            buf.push_str(&serde_json::Value::Object(obj).to_string());
            buf.push('\n');
        }
        std::fs::write(&new, &buf).map_err(|e| format!("write {}: {}", new, e))?;
        event_log::write_next_seq(&new, (max_seq + 1).max(1) as u64);
        // Reversible backup of the old heki store.
        let _ = std::fs::rename(&old, format!("{}.premigration", old));
        let after = event_log::load_states(&new).len();
        Ok((before, after))
    }

    /// The unconsolidated Log tail : the Event records that live in the shards
    /// but have NOT yet been folded into the global event.heki. The COMPLETE
    /// Log is the consolidated event.heki PLUS this tail — `verify-projection`
    /// needs both to reconstruct a LIVE aggregate, because a continuously-
    /// dispatching aggregate (e.g. the 2s InboxPoller) always has its freshest
    /// event sitting in a shard, ~consolidation-cadence ahead of event.heki.
    ///
    /// READ-ONLY : unlike run_consolidate, this neither writes the global Log
    /// nor saves the advanced checkpoint — it loads a private checkpoint copy,
    /// runs one merge_pass to collect the post-checkpoint records, and discards
    /// the mutated copy. So a verifier observes the tail without perturbing the
    /// live consolidate driver. The returned states carry the Event's fields in
    /// the same nested shape (delta{field,value}, sequence{value}, recorded_at)
    /// the consolidated reader produces, so the fold treats both uniformly.
    pub fn unconsolidated_log_tail(&self) -> Vec<AggregateState> {
        let es_key = repo_key(Some("EventSourcing"), "Event");
        let store_dir = match self.repositories.get(&es_key).and_then(|r| r.heki_path()) {
            Some(d) => d,
            None => return Vec::new(),
        };
        let shard_dir = std::path::Path::new(&store_dir).join("shards");
        let checkpoint = shard_dir.join(".merge.checkpoint.json");
        let shards = match event_merge::discover_shards(&shard_dir) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let mut cp = event_merge::load_checkpoint(&checkpoint)
            .unwrap_or_else(|_| event_merge::Checkpoint::new());
        let batch = match event_merge::merge_pass(&shards, &mut cp) {
            Ok(b) => b,
            Err(_) => return Vec::new(),
        };
        // Discard cp : read-only, no save_checkpoint / write_batch_to_global.
        batch
            .iter()
            .map(|rec| {
                let mut st = AggregateState::new("");
                for (k, v) in &rec.event {
                    st.set(k, json_to_value_recursive(v));
                }
                st
            })
            .collect()
    }
}
