//! Repository — in-memory aggregate storage with heki persistence
//!
//! Stores AggregateState instances by id. On every save, upserts
//! to a .heki store so state is shared with Miette's organs.
//!
//! Id dispatch (id_for_command): driven entirely by `identified_by`
//! from the aggregate IR — no defaults, no singleton fallback.
//!
//! Callers must always supply the id explicitly:
//!   identified_by :name  → pass name=heartbeat in command attrs
//!   identified_by :ref   → pass ref=abc123 in command attrs
//!
//! Without identified_by, mints a u64 counter on creation.
//!
//! Freshness (i517 dream-and-bug correspondence) :
//! `last_seen_mtime` tracks the heki file's mtime as of our last
//! load or save. `refresh_from_heki` stat()s the file and reloads
//! when disk has advanced — closing the cross-process staleness gap
//! a long-running daemon hits when a sibling process writes to the
//! same store. The bluebook contract for this lives in
//! runtime/storage/storage.bluebook (last_seen_mtime attribute,
//! RefreshIfStale command, RefreshOnPulse policy).
//!
//! Usage:
//!   let repo = Repository::new("Heartbeat", data_dir, Some("name".into()));
//!
//! [antibody-exempt: runtime aggregate store ;
//!  (a) identified_by dispatch drives natural-key vs counter-mint (i80) ;
//!  (b) cross-process freshness — load_persisted / save / refresh_from_heki
//!      track and re-read on mtime advance, honoring the storage.bluebook
//!      RefreshIfStale + RefreshOnPulse contract (i517 root cause).]

use super::repo_hydrate::to_json;
use super::AggregateState;
use super::Value;
use crate::heki;
use std::collections::HashMap;
use std::time::SystemTime;

pub struct Repository {
    pub(super) store: HashMap<String, AggregateState>,
    pub(super) next_id: u64,
    pub(super) aggregate_type: String,
    pub(super) data_dir: Option<String>,
    identified_by: Option<String>,
    /// Bounded context (bluebook namespace) — set by Runtime when the
    /// aggregate's IR carries one. Used to namespace the heki path
    /// (i142 Tier 2) so same-name aggregates in different contexts
    /// don't collide on storage. None = legacy aggregate with flat
    /// heki path (the pre-i142 default).
    pub(super) context: Option<String>,
    /// Last mtime we observed on disk for this repo's heki file. Used
    /// by `refresh_from_heki` to skip the read when nothing has changed
    /// since our last load/save. None until the first successful
    /// load_persisted ; updated on every save and every refresh-driven
    /// reload. Mirrors the `last_seen_mtime` attribute declared on the
    /// Repository aggregate in runtime/storage/storage.bluebook.
    pub(super) last_seen_mtime: Option<SystemTime>,
}

impl Repository {
    pub fn new(
        aggregate_type: &str,
        data_dir: Option<String>,
        identified_by: Option<String>,
    ) -> Self {
        Self::new_with_context(aggregate_type, data_dir, identified_by, None)
    }

    /// New repository with bounded-context awareness (i142 Tier 2).
    /// When context is set, the heki path is `<dir>/<context_snake>/<aggregate_snake>.heki`
    /// instead of the flat `<dir>/<aggregate_snake>.heki`. Migrates
    /// flat-path data into the context-prefixed path on first load
    /// (when target dir is empty and source file exists).
    pub fn new_with_context(
        aggregate_type: &str,
        data_dir: Option<String>,
        identified_by: Option<String>,
        context: Option<String>,
    ) -> Self {
        let mut repo = Repository {
            store: HashMap::new(),
            next_id: 1,
            aggregate_type: aggregate_type.to_string(),
            data_dir,
            identified_by,
            context,
            last_seen_mtime: None,
        };
        repo.load_persisted();
        repo
    }

    /// Resolve the id for a command dispatch.
    ///
    ///   identified_by + attr present → use attr value (explicit)
    ///   identified_by + attr absent + exactly 1 record → use existing id
    ///     (singleton fallback: loop/policy caller didn't pass the id but
    ///      the record already exists; use it rather than counter-minting a
    ///      second record each tick. Mirrors inject_refs logic in mod.rs.)
    ///   identified_by + attr absent + 0 or >1 records → counter-mint
    ///   identified_by absent → counter-mint
    pub fn id_for_command(&mut self, attrs: &HashMap<String, Value>) -> String {
        if let Some(ref key) = self.identified_by {
            if let Some(Value::Str(s)) = attrs.get(key) {
                return s.clone();
            }
            // Singleton fallback: no id attr but exactly one existing record.
            if self.store.len() == 1 {
                if let Some(existing) = self.store.values().next() {
                    return existing.id.clone();
                }
            }
        }
        let id = self.next_id;
        self.next_id += 1;
        id.to_string()
    }

    pub fn save(&mut self, state: AggregateState, ctx: heki::WriteContext<'_>) {
        self.store.insert(state.id.clone(), state);
        if let Some(ref dir) = self.data_dir {
            let path = self.heki_path_self(dir);
            if let Some(parent) = std::path::Path::new(&path).parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let mut heki_store = heki::Store::new();
            for (id, s) in &self.store {
                let mut rec = heki::Record::new();
                rec.insert("id".into(), serde_json::Value::String(id.clone()));
                for (key, val) in &s.fields {
                    rec.insert(key.clone(), to_json(val));
                }
                heki_store.insert(id.clone(), rec);
            }
            let _ = heki::write(&path, &heki_store, ctx);
            // Stamp last_seen_mtime to the post-write mtime so the next
            // refresh_from_heki tick sees no advance and skips the
            // re-read of our own write. Closes the read-our-own-write
            // round-trip waste a naive mtime gate would create.
            self.last_seen_mtime = self.current_disk_mtime();
        }
    }

    /// Remove a record from the in-memory store and persist the
    /// updated set back to heki. The companion to `save` for the
    /// `then_delete` mutation primitive ; only retire-style commands
    /// reach this path.
    ///
    /// Snapshots the heki file before deleting so the prior store can
    /// be recovered if the deletion was wrong. Snapshot failure is
    /// logged but does not block the delete — the dispatch path must
    /// stay live even if the snapshots dir is unwritable.
    pub fn delete(&mut self, id: &str, ctx: heki::WriteContext<'_>) {
        self.store.remove(id);
        if let Some(ref dir) = self.data_dir {
            // Context-aware path resolution (i142 Tier 2) — picks the
            // namespaced or flat heki path per the aggregate's
            // declared context.
            let path = self.heki_path_self(dir);
            // Snapshot before delete (i122 round 1, the heki snapshot
            // primitive) — the only destructive runtime call gets a
            // backup. Failures log but don't block the dispatch.
            match heki::snapshot(&path) {
                Ok(Some(snap)) => {
                    if std::env::var("HECKS_HEKI_AUDIT").ok().as_deref() == Some("1") {
                        eprintln!("[heki:snapshot] {} → {}", path, snap);
                    }
                }
                Ok(None) => {} // file didn't exist — nothing to snapshot
                Err(e) => eprintln!("[heki:snapshot] warning: {}", e),
            }
            let _ = heki::delete(&path, id, ctx);
            // Same freshness-bookkeeping as save : stamp the post-write
            // mtime so refresh_from_heki skips re-reading our own
            // delete on the next tick.
            self.last_seen_mtime = self.current_disk_mtime();
        }
    }

    /// Resolve the heki path for THIS repository — context-prefixed
    /// when context is set (i142 Tier 2), flat otherwise (legacy).
    /// Routes through the canonical `heki::path_for` helper (i145).
    pub(super) fn heki_path_self(&self, dir: &str) -> String {
        heki::path_for(dir, &self.aggregate_type, self.context.as_deref())
    }

    pub fn find(&self, id: &str) -> Option<&AggregateState> {
        self.store.get(id)
    }

    pub fn find_mut(&mut self, id: &str) -> Option<&mut AggregateState> {
        self.store.get_mut(id)
    }

    pub fn all(&self) -> Vec<&AggregateState> {
        self.store.values().collect()
    }

    pub fn count(&self) -> usize {
        self.store.len()
    }

    /// Inject an AggregateState into the in-memory store without going
    /// through the heki write path. Used by callers that hydrate the
    /// repository from an alternate storage backend (e.g. the wasm32
    /// worker's R2 read-on-boot path : `bin-buddy/worker/src/lib.rs`
    /// reads `state/<agg>.heki` from R2 and seeds the Repository here).
    ///
    /// Bumps `next_id` past the seeded id when it parses as a u64 — the
    /// same max-id walk `load_persisted` does for filesystem-backed
    /// boots — so subsequent counter-mints don't collide with the
    /// seeded record.
    pub fn seed_record(&mut self, state: AggregateState) {
        if let Ok(n) = state.id.parse::<u64>() {
            if n >= self.next_id {
                self.next_id = n + 1;
            }
        }
        self.store.insert(state.id.clone(), state);
    }

    /// Current `next_id` counter value. Used by external persistence
    /// layers (the wasm32 worker's R2 `state/_counters.heki` write) to
    /// snapshot counter state across requests so counter-minted ids
    /// don't reset to 1 on every cold boot.
    pub fn next_id_value(&self) -> u64 {
        self.next_id
    }

    /// Restore `next_id` from an external counter snapshot. Companion
    /// to `next_id_value` ; the worker reads `_counters.heki` at boot
    /// and calls this for each aggregate that had a persisted counter.
    /// No-op when `value` is `<=` the current next_id (the seeded
    /// records already pushed it past `value`).
    pub fn set_next_id(&mut self, value: u64) {
        if value > self.next_id {
            self.next_id = value;
        }
    }
}
