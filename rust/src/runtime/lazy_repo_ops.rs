//! lazy_repo_ops — the LazyRepository operation surface : is_hydrated,
//! find / find_mut / all / count / query / next_id_value / id_for_command
//! / save and friends, each delegating through the lazy-hydration cell to
//! the backing Repository or PersistenceAdapter. Construction + hydration
//! plumbing stay in lazy_repository.rs.
//!
//! Cask extracted VERBATIM from runtime/lazy_repository.rs (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/lazy_repo_ops.rs — kernel-floor
//!  i-lazy repository surface, relocated verbatim from lazy_repository.rs
//!  blanket.]

use super::lazy_repository::{Backend, LazyRepository};

use super::{AggregateState, Value};
use crate::heki;
use std::collections::HashMap;

impl LazyRepository {
    /// Whether the underlying repository has been hydrated yet. Used by
    /// `hydrate_all` to skip already-warm repos and by the teardown
    /// story (un-hydrated cells drop for free).
    pub fn is_hydrated(&self) -> bool {
        match &self.backend {
            Backend::Heki { cell, .. } | Backend::Memory { cell, .. } | Backend::AppendLog { cell, .. } => cell.get().is_some(),
            // SQL is eager — built at boot, so always hydrated.
            Backend::Adapter { .. } => true,
        }
    }

    // ----- forwarded repository surface (read : &self) -----

    pub fn find(&self, id: &str) -> Option<&AggregateState> {
        if self.is_adapter() { self.adapter().find(id) } else { self.repo().find(id) }
    }

    pub fn all(&self) -> Vec<&AggregateState> {
        if self.is_adapter() { self.adapter().all() } else { self.repo().all() }
    }

    pub fn count(&self) -> usize {
        if self.is_adapter() { self.adapter().count() } else { self.repo().count() }
    }

    /// The where() pushdown seam. SQL routes to the connection-executed,
    /// injection-safe parameterized prefilter ; AppendLog routes to the
    /// filtered streaming scan of the Event Log (hydrate only matching
    /// lines, not the whole growing Log). Both return OWNED candidate
    /// states. Heki/Memory have no pushdown, so they return `None` and the
    /// caller keeps the in-memory `all()` + `where_matches` oracle path (no
    /// clone regression) ; AppendLog also returns `None` when no clause is
    /// pushable, falling back to the cell-backed `all()`. The oracle
    /// re-applies every clause regardless, so a `Some` prefilter can only
    /// narrow — parity holds by construction.
    pub fn query(
        &self,
        wheres: &[crate::ir::WhereClause],
        attrs: &HashMap<String, String>,
    ) -> Option<Vec<AggregateState>> {
        match &self.backend {
            Backend::Adapter { .. } => self.adapter().query(wheres, attrs),
            Backend::AppendLog { data_dir, context, .. } => {
                let dir = data_dir.as_ref()?;
                let path = super::event_log::global_path(dir, context.as_deref());
                super::event_log_query::load_filtered(&path, wheres, attrs)
            }
            _ => None,
        }
    }

    pub fn next_id_value(&self) -> u64 {
        if self.is_adapter() { self.adapter().next_id_value() } else { self.repo().next_id_value() }
    }

    // ----- forwarded repository surface (mutate : &mut self) -----

    pub fn find_mut(&mut self, id: &str) -> Option<&mut AggregateState> {
        if self.is_adapter() { self.adapter_mut().find_mut(id) } else { self.repo_mut().find_mut(id) }
    }

    pub fn id_for_command(&mut self, attrs: &HashMap<String, Value>) -> String {
        if self.is_adapter() { self.adapter_mut().id_for_command(attrs) } else { self.repo_mut().id_for_command(attrs) }
    }

    pub fn save(&mut self, state: AggregateState, ctx: heki::WriteContext<'_>) {
        // AppendLog : append-not-upsert. The Event Log's save persists one
        // immutable shard record instead of overwriting a per-id row.
        if let Backend::AppendLog { data_dir, .. } = &self.backend {
            Self::append_log_save(&data_dir.clone(), &state);
            return;
        }
        if self.is_adapter() { self.adapter_mut().save(state, ctx) } else { self.repo_mut().save(state, ctx) }
    }

    /// AppendLog save — build one immutable shard record from the Event state
    /// and append it to THIS process's private shard (a `shards/` sibling of
    /// the merged event.heki). event_id + sequence were reserved at dispatch
    /// (record_event_append) and ride in `state`, so `append_record` writes
    /// the pre-stamped record. If the sink isn't open yet (a direct Append
    /// with no prior reserve), fall back to append_to_process_shard, which
    /// opens the sink + stamps a fresh id.
    fn append_log_save(data_dir: &Option<String>, state: &AggregateState) {
        use super::event_shard::{self, ShardRecord};
        let dir = match data_dir {
            Some(d) => d,
            None => return, // memory-backed AppendLog has no disk shard target
        };
        let shard_dir = std::path::Path::new(dir).join("shards");
        let s = |k: &str| state.get(k).as_str().unwrap_or_default().to_string();
        let seq = match state.get("sequence") {
            Value::Map(m) => m.get("value").and_then(|v| v.as_int()).unwrap_or(0),
            Value::Int(i) => *i,
            _ => 0,
        } as u64;
        // The event payload is the FULL Event AggregateState, serialized — the
        // ONE source of the Event shape (no parallel field list to drift). The
        // merge writes this verbatim to the global Log, so a reader sees the
        // real nested Event (command{verb,inputs}, delta{field,value}, …).
        let mut event = serde_json::Map::new();
        for (k, v) in &state.fields {
            event.insert(k.clone(), super::value_to_json(v));
        }
        let rec = ShardRecord {
            shard: event_shard::process_shard_id().to_string(),
            seq,
            ts: s("recorded_at"),
            event_id: s("event_id"),
            event,
        };
        if !event_shard::append_record(&rec) {
            let _ = event_shard::append_to_process_shard(&shard_dir, rec);
        }
    }

    pub fn delete(&mut self, id: &str, ctx: heki::WriteContext<'_>) {
        if self.is_adapter() { self.adapter_mut().delete(id, ctx) } else { self.repo_mut().delete(id, ctx) }
    }

    /// Re-read from disk when a sibling process advanced the store.
    /// No-op for the SQL backend — every read goes to the live db
    /// connection, so there's no stale in-memory snapshot to refresh
    /// against (the heki cross-process freshness concern doesn't apply).
    pub fn refresh_from_heki(&mut self) {
        // AppendLog owns the append-only Log : re-seed (invalidate the cell so the
        // next read re-materializes from event.log) ONLY when the Log has grown
        // since we last read it. A cheap stat until the merge actually appends.
        if let Backend::AppendLog { cell, data_dir, context, log_mtime, .. } = &mut self.backend {
            if let Some(dir) = data_dir {
                let path = super::event_log::global_path(dir, context.as_deref());
                let cur = super::event_log::mtime(&path);
                if cur != log_mtime.get() {
                    let _ = cell.take();
                    log_mtime.set(cur);
                }
            }
            return;
        }
        if !self.is_adapter() { self.repo_mut().refresh_from_heki() }
    }

    pub fn seed_record(&mut self, state: AggregateState) {
        if self.is_adapter() { self.adapter_mut().seed_record(state) } else { self.repo_mut().seed_record(state) }
    }

    pub fn set_next_id(&mut self, value: u64) {
        if self.is_adapter() { self.adapter_mut().set_next_id(value) } else { self.repo_mut().set_next_id(value) }
    }
}
