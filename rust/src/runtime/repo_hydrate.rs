//! repo_hydrate — the Repository's disk half : current_disk_mtime (the
//! freshness baseline), refresh_from_heki (sibling-process pickup gated on
//! mtime advance), load_persisted (the boot read), and the Value↔JSON
//! codecs (to_json / from_json — nested VOs stay maps, never stringified).
//! The in-memory op surface + save/delete stay in repository.rs.
//!
//! Cask extracted VERBATIM from runtime/repository.rs (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/repo_hydrate.rs — kernel-floor
//!  repository hydration, relocated verbatim from repository.rs blanket.]

use super::repository::Repository;
use super::{AggregateState, Value};
use crate::heki;
use std::time::SystemTime;

impl Repository {
    /// File mtime for our heki path, when the file exists. Used by
    /// load / save / refresh to track whether the in-memory store is
    /// current with disk. Returns None when data_dir is unset, the
    /// file doesn't exist yet, or the stat call fails — all of which
    /// the freshness logic treats as "no recorded mtime, fall through
    /// to a read."
    pub(super) fn current_disk_mtime(&self) -> Option<SystemTime> {
        let dir = self.data_dir.as_ref()?;
        let path = self.heki_path_self(dir);
        std::fs::metadata(&path).and_then(|m| m.modified()).ok()
    }

    /// Re-read the heki file when disk mtime has advanced past our
    /// last_seen_mtime — i.e. another process wrote since our last
    /// load or save. No-op when disk is unchanged (just a stat call).
    /// Honors the `RefreshIfStale` command declared on the Repository
    /// aggregate in runtime/storage/storage.bluebook ; the
    /// `RefreshOnPulse` policy on BodyPulse fans this out across every
    /// repo via Runtime::refresh_repositories_from_heki on each tick.
    pub fn refresh_from_heki(&mut self) {
        let Some(disk_mtime) = self.current_disk_mtime() else { return };
        let stale = match self.last_seen_mtime {
            Some(seen) => disk_mtime > seen,
            None => true,
        };
        if !stale { return; }
        // Drop in-memory store and reload from disk. Counter-mint
        // state (next_id) is rebuilt by load_persisted's max-id walk.
        self.store.clear();
        self.next_id = 1;
        self.load_persisted();
    }

    pub(super) fn load_persisted(&mut self) {
        let Some(ref dir) = self.data_dir else { return };
        let path = self.heki_path_self(dir);
        // i142 Tier 2 — auto-migration : when reading from the new
        // context-prefixed path returns nothing, but the flat
        // pre-context path has data, MOVE the flat file into the
        // context dir. One-shot lazy migration ; runs on the first
        // load of each aggregate after Tier 2 ships. Subsequent
        // reads/writes go through the new path natively.
        if self.context.is_some() {
            let new_records = heki::read(&path).unwrap_or_default();
            if new_records.is_empty() {
                let flat_path = heki::path_for(dir, &self.aggregate_type, None);
                if std::path::Path::new(&flat_path).exists() && flat_path != path {
                    if let Some(parent) = std::path::Path::new(&path).parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    let _ = std::fs::rename(&flat_path, &path);
                }
            }
        }
        let records = heki::read(&path).unwrap_or_default();
        for rec in records.values() {
            let id = rec.get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("1")
                .to_string();
            if let Ok(n) = id.parse::<u64>() {
                if n >= self.next_id { self.next_id = n + 1; }
            }
            let mut state = AggregateState::new(&id);
            for (key, val) in rec {
                if key != "id" && key != "created_at" && key != "updated_at" {
                    state.set(key, from_json(val));
                }
            }
            self.store.insert(id, state);
        }
        // Quiet by default — every dispatch boots a fresh runtime and
        // would otherwise spam dozens of "loaded N records from disk"
        // lines (visible especially in the narrow PostToolUse hook
        // output column). Set HECKS_REPO_VERBOSE=1 to see them when
        // debugging boot-time loading.
        if !self.store.is_empty()
            && std::env::var("HECKS_REPO_VERBOSE").ok().as_deref() == Some("1")
        {
            eprintln!("  loaded {} {} records from disk",
                self.store.len(), self.aggregate_type);
        }
        // Stamp the freshness baseline. If the file doesn't exist yet
        // (an aggregate with no persisted records), we leave
        // last_seen_mtime as None so the first refresh treats it as
        // stale — once a sibling process writes, we'll pick it up.
        self.last_seen_mtime = self.current_disk_mtime();
    }
}

pub(super) fn to_json(val: &Value) -> serde_json::Value {
    match val {
        Value::Str(s) => serde_json::Value::String(s.clone()),
        Value::Int(n) => serde_json::json!(*n),
        Value::Bool(b) => serde_json::json!(*b),
        Value::Null => serde_json::Value::Null,
        Value::List(items) => serde_json::json!(items.iter().map(to_json).collect::<Vec<_>>()),
        Value::Map(m) => {
            let obj: serde_json::Map<String, serde_json::Value> =
                m.iter().map(|(k, v)| (k.clone(), to_json(v))).collect();
            serde_json::Value::Object(obj)
        }
    }
}

pub(super) fn from_json(val: &serde_json::Value) -> Value {
    match val {
        serde_json::Value::String(s) => Value::Str(s.clone()),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() { Value::Int(i) }
            else { Value::Str(n.to_string()) }
        }
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Array(a) => Value::List(a.iter().map(from_json).collect()),
        serde_json::Value::Object(m) => {
            Value::Map(m.iter().map(|(k, v)| (k.clone(), from_json(v))).collect())
        }
    }
}
