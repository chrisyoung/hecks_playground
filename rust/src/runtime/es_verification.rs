//! es_verification — the Log maintenance triggers : run_consolidate_if
//! (fires the merge fold IFF the dispatched command is
//! EventSourcing::Consolidation.Consolidate — the run_merge daemon's
//! replacement) and run_verification_if (the standing derivability
//! invariant, fired for EventSourcing::Verification.Verify). Both are
//! no-ops for every other command ; called from dispatch_inner so the
//! manual path AND the driver's dispatch_cascade reach them. Inherent
//! `impl Runtime` methods in a child module, same contract as
//! event_sourcing.rs.
//!
//! Cask extracted VERBATIM from runtime/event_sourcing.rs (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/es_verification.rs — kernel-floor
//!  runtime, relocated verbatim from event_sourcing.rs blanket.]

use super::*;

impl Runtime {
    /// Event Log consolidation trigger — fires the merge fold IFF the
    /// dispatched command is the Consolidation maintenance command. Called from
    /// dispatch_inner (so BOTH the manual path AND the driver's dispatch_cascade
    /// reach it). A no-op for every other command. Replaces the run_merge
    /// daemon : `storehouse drive` fires Consolidation.Consolidate on an
    /// interval, and this runs one merge pass.
    pub(super) fn run_consolidate_if(&self, command_name: &str) {
        if command_name == "EventSourcing::Consolidation.Consolidate" {
            self.run_consolidate();
        }
    }

    /// DERIVABILITY AS A STANDING INVARIANT — the sibling of run_consolidate_if.
    /// Fires when the dispatched command is the Verification trigger (from the
    /// ProjectionVerification Driver, or by hand) ; a no-op for everything else.
    ///
    /// `verify-projection` proved that current state is derivable from the Log —
    /// the claim the whole substrate rests on — but only when an operator ran it.
    /// A property that must hold CONTINUOUSLY has to be measured continuously, so
    /// the verdict becomes queryable domain state (Verification.Drifted) rather
    /// than console output nobody is watching.
    pub(super) fn run_verification_if(&mut self, command_name: &str) {
        if command_name != "EventSourcing::Verification.Verify" {
            return;
        }
        // The SAME gauge `storehouse verify-projection` reports — one
        // implementation, so the standing invariant and the operator's command can
        // never disagree about what derivable means.
        let measured = projection_fold::measure_persistent(self, 2);
        // COMPLETENESS, measured alongside derivability — two DIFFERENT properties,
        // and the fold gauge is structurally blind to this one : a dropped
        // event-row carries an empty delta, contributes nothing to the fold, and
        // leaves fold(Log) == store looking perfect. Non-zero here means events
        // happened that the Log does not contain.
        let missing = self.missing_event_rows();
        let (status, drift, transient, checked, detail) = match measured {
            None => {
                let s = if missing > 0 { "drifted" } else { "empty" };
                (s, 0usize, 0usize, 0usize, String::from("-"))
            }
            Some((persistent, transient, m)) => {
                // EITHER failure condemns the verdict : state that cannot be
                // rederived, OR events that were never recorded.
                let status = if persistent.is_empty() && missing == 0 { "derivable" } else { "drifted" };
                // A handful of triples is enough to act on ; the whole set could be
                // unbounded, and this is domain state, not a dump.
                let mut keys: Vec<String> = persistent
                    .keys()
                    .map(|(a, i, f)| format!("{a}::{i}::{f}"))
                    .collect();
                keys.sort();
                let detail = if keys.is_empty() {
                    String::from("-")
                } else {
                    let shown = keys.len().min(5);
                    let mut d = keys[..shown].join(", ");
                    if keys.len() > shown {
                        d.push_str(&format!(" (+{} more)", keys.len() - shown));
                    }
                    d
                };
                (status, persistent.len(), transient, m.total_fields, detail)
            }
        };
        let mut attrs: HashMap<String, Value> = HashMap::new();
        attrs.insert("verification_id".to_string(), Value::Str("projection".to_string()));
        attrs.insert("status".to_string(), Value::Str(status.to_string()));
        attrs.insert("checked_at".to_string(), Value::Str(crate::clock::now_iso()));
        attrs.insert("drift_fields".to_string(), Value::Str(drift.to_string()));
        attrs.insert("transient".to_string(), Value::Str(transient.to_string()));
        attrs.insert("fields_checked".to_string(), Value::Str(checked.to_string()));
        attrs.insert("missing_rows".to_string(), Value::Str(missing.to_string()));
        attrs.insert("detail".to_string(), Value::Str(detail));
        // Into the COLLABORATOR, where the EventSourcing chapter lives — same as
        // the Append writer. Ungated core dispatch : the recursion guard skips the
        // EventSourcing aggregates, so recording a verdict cannot re-enter this.
        let _ = command_dispatch::dispatch(
            self.framework_mut(),
            "EventSourcing::Verification.Record",
            attrs,
        );
    }

    /// One merge pass : fold this realm's per-process Event shards into the
    /// global ordered Log, writing WHERE the Event repository reads (so a
    /// reader sees the consolidated log — the writer/reader never diverge).
    /// Kernel-floor persistence IO : the shard byte-IO + the fold live in
    /// event_shard / event_merge (siblings of heki.rs) ; this only wires the
    /// realm's paths to them. The checkpoint advances only after a durable
    /// global write, so a failed write retries next pass and a crash mid-pass
    /// replays idempotently (dedup by event_id). No-op when the Event repo is
    /// memory-backed (no shard dir).
        fn run_consolidate(&self) {
        let es_key = repo_key(Some("EventSourcing"), "Event");
        let store_dir = match self.repositories.get(&es_key).and_then(|r| r.heki_path()) {
            Some(d) => d,
            None => return,
        };
        let shard_dir = std::path::Path::new(&store_dir).join("shards");
        // The global Log path — the APPEND-ONLY JSONL file the AppendLog adapter
        // reads (event_log::global_path with the EventSourcing context), so the
        // merge writer and the aggregate reader never diverge. NOT a heki store :
        // heki is for snapshots, the LOG is append-only (event_log.rs).
        let global = event_log::global_path(&store_dir, Some("EventSourcing"));
        let checkpoint = shard_dir.join(".merge.checkpoint.json");
        let shards = match event_merge::discover_shards(&shard_dir) {
            Ok(s) => s,
            Err(_) => return,
        };
        let mut cp = event_merge::load_checkpoint(&checkpoint)
            .unwrap_or_else(|_| event_merge::Checkpoint::new());
        if let Ok(batch) = event_merge::merge_pass(&shards, &mut cp) {
            // Adopt the advanced checkpoint only after a durable global write
            // (empty batch = nothing to write, offsets unchanged — safe to save).
            let write_ok = batch.is_empty()
                || event_merge::write_batch_to_global(&global, &batch).is_ok();
            if write_ok {
                let _ = event_merge::save_checkpoint(&checkpoint, &cp);
                // Reclaim the per-process shards this merge has now FULLY
                // folded whose owning process is DEAD — the GC half of
                // consolidation (every dispatch process leaves an orphan
                // shard ; without this the data root's .../shards grows without
                // bound). Only when the live-process set is TRUSTWORTHY :
                // live_pids() returns None on a failed/empty ps and we
                // reclaim nothing, never risking a live process's open
                // shard. Persist the pruned checkpoint when anything went.
                if let Some(live) = live_pids() {
                    let n = event_merge::reclaim_consumed_shards(
                        &shards,
                        &mut cp,
                        &|pid| live.contains(&pid),
                    );
                    if n > 0 {
                        let _ = event_merge::save_checkpoint(&checkpoint, &cp);
                    }
                }
            }
        }
    }
}
