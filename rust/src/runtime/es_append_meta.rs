//! es_append_meta — the append path's bookkeeping surface : COMPLETENESS
//! (owes_event_row — the legitimate no-row-owed reasons, and only those —
//! plus the missing_event_rows gap counter) and Phase-4 CAUSATION lineage
//! (note_last_event, cause_for_cascade, mint_correlation_id). The writer
//! itself (record_event_append) stays in event_sourcing.rs. Inherent
//! `impl Runtime` methods in a child module, same contract as
//! event_sourcing.rs.
//!
//! Cask extracted VERBATIM from runtime/event_sourcing.rs (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/es_append_meta.rs — kernel-floor
//!  runtime, relocated verbatim from event_sourcing.rs blanket.]

use super::*;

impl Runtime {
    /// COMPLETENESS — does this emitted domain event OWE the Log an event-row?
    ///
    /// Called from the dispatch path BEFORE the writer runs, so that a writer
    /// which returns early still leaves the debt visible. It answers the question
    /// the writer's own early-returns cannot be trusted to answer about themselves.
    ///
    /// The LEGITIMATE reasons no row is owed, and only these :
    ///   * the aggregate is not event_sourced (and the global override is off) —
    ///     nothing is logged for it at all ;
    ///   * it is runtime MECHANISM, not domain intent (Cascade / Process / …) ;
    ///   * it is an EventSourcing aggregate — Append must not append ;
    ///   * the Event repo is memory-backed, so there is no shard to write to.
    /// Every OTHER emitted event owes exactly one row. A guard that swallows one
    /// — as the empty-delta return did — now shows up as a gap rather than as
    /// silence.
    pub(crate) fn owes_event_row(&self, aggregate_type: &str) -> bool {
        if std::env::var("HECKS_EVENT_SOURCING").is_err()
            && !self.aggregate_is_event_sourced(aggregate_type)
        {
            return false;
        }
        if projection_fold::is_infra_mechanism(aggregate_type) {
            return false;
        }
        if self
            .domain
            .aggregates
            .iter()
            .any(|a| a.name == aggregate_type && a.context.as_deref() == Some("EventSourcing"))
        {
            return false;
        }
        // No shard target (memory-backed Event) => nothing is written by design.
        let es_key = repo_key(Some("EventSourcing"), "Event");
        self.framework
            .as_ref()
            .and_then(|fw| fw.repositories.get(&es_key))
            .and_then(|r| r.heki_path())
            .is_some()
    }

    /// The emitted domain events the Log does not contain. ALWAYS ZERO on a
    /// healthy runtime — any other number names events that happened and were not
    /// recorded, which is the one failure a source of truth cannot have.
    pub fn missing_event_rows(&self) -> u64 {
        self.event_rows_owed.saturating_sub(self.event_rows_written)
    }

    /// Phase-4 causation — note the Log event_id just recorded for a domain
    /// aggregate, so a later cascade off it can stamp `causation_id`. Keyed
    /// "<type>::<id>" ; the latest write wins (a command's last delta-event is
    /// the representative cause). In-process, transient.
    pub(super) fn note_last_event(&mut self, agg_type: &str, agg_id: &str, event_id: &str) {
        self.last_event_id_by_agg
            .insert(format!("{}::{}", agg_type, agg_id), event_id.to_string());
    }

    /// Phase-4 causation — the triggering event_id for a cascaded dispatch. A
    /// cascade carries its upstream (type, id) as a hint ; the cause is the
    /// last Log event recorded for that aggregate (the event whose processing
    /// fired this cascade). Empty for a root dispatch (no hint) or an upstream
    /// that recorded no events — both leave causation_id empty, where
    /// CausationTrace stops.
    pub(crate) fn cause_for_cascade(&self, hint: &Option<(String, String)>) -> String {
        hint.as_ref()
            .and_then(|(t, i)| self.last_event_id_by_agg.get(&format!("{}::{}", t, i)).cloned())
            .unwrap_or_default()
    }

    /// Mint a fresh per-FLOW correlation id at the ROOT of a dispatch. Every
    /// event in one flow — the root command plus every cascade it triggers —
    /// shares this id, so the Log can gather a whole business flow (a transfer
    /// saga) as one unit (the ByCorrelation facet). Process-unique : a monotonic
    /// counter guarantees no two roots collide within a run ; the wall-clock
    /// second prefix keeps the id readable and distinct across runs, and the
    /// root verb tail names WHICH flow it is. wasm-safe clock (i630).
    pub(crate) fn mint_correlation_id(&self, command_name: &str) -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static CORRELATION_SEQ: AtomicU64 = AtomicU64::new(0);
        let n = CORRELATION_SEQ.fetch_add(1, Ordering::Relaxed);
        let secs = crate::clock::now_duration().as_secs();
        let verb = command_name.rsplit('.').next().unwrap_or(command_name);
        format!("corr-{:x}-{:x}-{}", secs, n, verb)
    }
}
