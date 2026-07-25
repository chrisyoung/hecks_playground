//! es_replay — the READ SIDE of event sourcing (stage 4) : derive every
//! event-sourced aggregate's state FROM the Log at boot
//! (hydrate_event_sourced_from_log, overlay semantics — Log opinions win,
//! born-with defaults survive), plus the snapshot cadence
//! (snapshot_threshold, maybe_capture_snapshot) and the
//! CURRENT_STATE_PROJECTION name. Inherent `impl Runtime` methods in a
//! child module, same contract as event_sourcing.rs.
//!
//! Cask extracted VERBATIM from runtime/event_sourcing.rs (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/es_replay.rs — kernel-floor runtime,
//!  relocated verbatim from event_sourcing.rs blanket.]

use super::*;

impl Runtime {
    /// STAGE 4 — THE READ SIDE. Derive every event-sourced aggregate's state
    /// FROM the Log, rather than merely mirroring it into a parallel store.
    ///
    /// This is what makes the Log authoritative rather than a second record kept
    /// alongside the real one. Until now the eager heki current-state write WAS
    /// the read : if it was stale, truncated, or lost, the read was wrong and the
    /// Log — which had the facts — was never consulted. Now the state a caller
    /// reads is the state the Log says it is.
    ///
    /// `load = Snapshot@watermark + fold_forward(tail)` : the projection is read
    /// through `ReadForward`, so it is O(tail) rather than O(whole-log) (that is
    /// what Stage 3's capture bought) and there is ONE fold shared with the query,
    /// never a second that could drift.
    ///
    /// IDEMPOTENT BY CONSTRUCTION — why overlaying is safe. A delta records the
    /// field's FULL post-command value, never an increment, so applying it once or
    /// five times lands on the same value. That is also why this OVERLAYS the
    /// loaded record instead of replacing it : fields the Log has an opinion about
    /// win, and fields it has never seen (defaults an aggregate was born with)
    /// survive untouched.
    ///
    /// It also dissolves the synchronicity window the arc's locked decision names:
    /// a crash between the state save and the Log append can no longer strand a
    /// recorded event, because the next load re-derives from the Log.
    pub fn hydrate_event_sourced_from_log(&mut self) {
        let name = Self::CURRENT_STATE_PROJECTION;
        let mut params: HashMap<String, String> = HashMap::new();
        params.insert("projection_name".to_string(), name.to_string());
        let folded = self.framework_mut().resolve_query("ReadForward", &params);
        let rows = match folded.get("state").and_then(|s| s.as_array()) {
            Some(rows) if !rows.is_empty() => rows.clone(),
            _ => return,
        };

        // Regroup the flat projection rows ("<agg>::<id>::<field>") per instance.
        // rsplit for the FIELD and split-once for the AGG, so an id containing
        // "::" cannot smear the parse.
        let mut per_instance: HashMap<(String, String), Vec<(String, String)>> = HashMap::new();
        for row in &rows {
            let key = match row.get("key").and_then(|k| k.as_str()) {
                Some(k) => k,
                None => continue,
            };
            let value = row.get("value").and_then(|v| v.as_str()).unwrap_or_default();
            let (agg, rest) = match key.split_once("::") {
                Some(p) => p,
                None => continue,
            };
            let (id, field) = match rest.rsplit_once("::") {
                Some(p) => p,
                None => continue,
            };
            if field.is_empty() {
                continue;
            }
            per_instance
                .entry((agg.to_string(), id.to_string()))
                .or_default()
                .push((field.to_string(), value.to_string()));
        }

        for ((agg, id), fields) in per_instance {
            if !self.aggregate_is_event_sourced(&agg) {
                continue;
            }
            let key = match repo_lookup_key(&self.repositories, &agg) {
                Some(k) => k,
                None => continue,
            };
            // Overlay onto what is already loaded, so Log-known fields win and
            // Log-unknown ones (defaults) survive.
            let mut record = self
                .repositories
                .get(&key)
                .and_then(|r| r.find(&id))
                .cloned()
                .unwrap_or_else(|| AggregateState::new(&id));
            for (field, raw) in fields {
                // The Log stores each delta value as COMPACT JSON, so a Money map
                // and a ledger list decode back TYPED — not as the string that
                // rendered them.
                record.set(&field, super::value_from_json_str(&raw));
            }
            if let Some(repo) = self.repositories.get_mut(&key) {
                repo.seed_record(record);
            }
        }
    }

    /// The trivial per-aggregate current-state projection — the one
    /// `Snapshot.ReadForward` serves (Row key "agg::id::field"). Named once here
    /// so the writer's capture and the reader's fold can never disagree about
    /// WHICH projection the snapshot caches.
    pub(crate) const CURRENT_STATE_PROJECTION: &'static str = "current_state";

    /// How far the Log may run past the snapshot before we re-capture.
    ///
    /// A THRESHOLD, not a timer — the bluebook is explicit twice over ("capture
    /// by THRESHOLD (every N events) or ON-DEMAND, not by time"), and it is right
    /// on the merits : a clock re-captures identical state when nothing happened,
    /// and falls behind exactly when it matters (a burst), because read cost
    /// tracks LOG GROWTH, not elapsed time. Tying capture to growth bounds the
    /// fall-forward tail directly. `HECKS_SNAPSHOT_EVERY` overrides it (0 = never
    /// capture), so a deployment can trade write cost against read cost.
    fn snapshot_threshold() -> i64 {
        std::env::var("HECKS_SNAPSHOT_EVERY")
            .ok()
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(100)
    }

    /// Capture the current-state snapshot IFF the Log has grown `threshold`
    /// events past the last watermark. Runs on the framework COLLABORATOR, where
    /// both Event and Snapshot live.
    ///
    /// The capture IS the read : it materialises exactly what `ReadForward` would
    /// compute right now (cached rows + the folded tail) and stores it at the new
    /// watermark, so a snapshot can never mean something different from the query
    /// that consumes it — there is one fold, not two. Afterwards the tail is
    /// empty, so the next read is O(1) instead of O(log).
    ///
    /// Safe by construction : a snapshot is a CACHE, never the source of truth.
    /// If this never runs, every read simply folds a longer tail. So a failed or
    /// skipped capture costs time, never correctness.
    ///
    /// KNOWN LIMIT : `sequence` is the per-PROCESS shard sequence until the merge
    /// assigns the global order, so a watermark captured in one process is only
    /// meaningful against that process's ordering. `ReadForward` already carries
    /// this property ; consolidating first is what makes it global.
    pub(super) fn maybe_capture_snapshot(&mut self) {
        let threshold = Self::snapshot_threshold();
        if threshold <= 0 {
            return;
        }
        let name = Self::CURRENT_STATE_PROJECTION;
        let fw = self.framework_mut();

        let head = fw
            .all_qualified(Some("EventSourcing"), "Event")
            .iter()
            .map(|e| match e.get("sequence") {
                Value::Int(i) => *i,
                Value::Map(m) => m.get("value").and_then(|v| v.as_int()).unwrap_or(0),
                _ => 0,
            })
            .max()
            .unwrap_or(0);
        let watermark = match fw.find("Snapshot", name).map(|s| s.get("watermark").clone()) {
            Some(Value::Int(i)) => i,
            Some(Value::Map(m)) => m.get("value").and_then(|v| v.as_int()).unwrap_or(0),
            _ => 0,
        };
        if head - watermark < threshold {
            return;
        }

        // The fold is the READ — one implementation, shared.
        let mut params: HashMap<String, String> = HashMap::new();
        params.insert("projection_name".to_string(), name.to_string());
        let folded = fw.resolve_query("ReadForward", &params);
        let rows: Vec<Value> = folded
            .get("state")
            .and_then(|s| s.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|r| {
                        let k = r.get("key")?.as_str()?.to_string();
                        let v = r.get("value")?.as_str().unwrap_or_default().to_string();
                        let mut row = HashMap::new();
                        row.insert("key".to_string(), Value::Str(k));
                        row.insert("value".to_string(), Value::Str(v));
                        Some(Value::Map(row))
                    })
                    .collect()
            })
            .unwrap_or_default();

        let mut watermark_vo = HashMap::new();
        watermark_vo.insert("value".to_string(), Value::Int(head));
        let mut attrs = HashMap::new();
        attrs.insert("projection_name".to_string(), Value::Str(name.to_string()));
        attrs.insert("watermark".to_string(), Value::Map(watermark_vo));
        attrs.insert("state".to_string(), Value::List(rows));
        let _ = command_dispatch::dispatch(fw, "EventSourcing::Snapshot.Capture", attrs);
    }

}
