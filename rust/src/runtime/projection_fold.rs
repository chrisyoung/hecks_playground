//! projection_fold — the trivial per-aggregate projection : fold the Event Log
//! back into current state. This is what makes "state-as-projection" PROVABLE :
//! current state is DERIVABLE from the Log (the source of truth), not just an
//! independently-saved store. `verify-projection` folds the Log and asserts it
//! reconstructs the live store, flipping the Log from "parallel record" to
//! "source of truth."
//!
//! [antibody-exempt: rust/src/runtime/projection_fold.rs — kernel-floor read-
//!  model fold. The CONCEPT is conceived in the EventSourcing bluebook
//!  (Projection.Apply — the honest delta fold ; Event.Replay — the Log
//!  surface) ; this is its hand-written runtime realization until the
//!  projection layer is generated from that chapter. Sibling of the kernel-
//!  floor event_shard / event_merge IO.]
//!
//! The fold is the simplest reducer the delta shape allows : each Event's delta
//! is (field, value) carrying the field's FULL post-command value, so folding =
//! "for each event in sequence order, set field := value." Last write per field
//! wins = current value — the trivial per-aggregate projection the EventSourcing
//! vision names ("current-state is just the trivial per-aggregate projection").
//!
//! Faithful values : the Log stores each delta value as COMPACT JSON
//! (record_event_append wrote `value_to_json_string(&value)`), so a Money Map
//! and a ledger List survive — the old `value.to_string()` rendered them as the
//! lossy `"{N fields}"` / `"[N items]"`. The fold carries that JSON string per
//! field ; the derivability check (`verify-projection`) decodes it back with
//! `value_from_json_str` and compares the typed `Value` STRUCTURALLY to the live
//! store (order-independent), so a rich aggregate reconstructs exactly and a
//! legacy lossy delta drifts loudly.

use super::{AggregateState, Value};
use std::collections::HashMap;

/// Reconstructed field map for one aggregate instance : field -> stringified value.
pub type ReconstructedState = HashMap<String, String>;

/// Where a folded field and the live store disagree :
/// (aggregate, id, field) -> (what the Log folds to, what the store holds).
pub type DriftMap = HashMap<(String, String, String), (String, String)>;

/// One instance's ordered delta rows during the fold :
/// (recorded_at, sequence, field, value). Ordered by recorded_at with sequence
/// as the tiebreak — see the note in `fold_event_log`.
type DeltaRows = Vec<(String, i64, String, String)>;

/// Fold the global Log's Event records into per-instance reconstructed state,
/// keyed by (aggregate_name, aggregate_id). Pure : groups by instance, orders
/// by sequence, applies field := value (last write wins). Events with no delta
/// field are skipped.
pub fn fold_event_log(
    events: &[&AggregateState],
) -> HashMap<(String, String), ReconstructedState> {
    // (agg_name, agg_id) -> [(recorded_at, sequence, field, value)]
    // Order key is recorded_at (wall-clock), NOT sequence : the COMPLETE log
    // mixes consolidated events (authoritative GLOBAL sequence, assigned at
    // merge) with the unconsolidated shard tail (per-process sequence, not yet
    // globally ordered). Those two sequence scales are not comparable, so
    // folding by sequence would sort a fresh tail event (small per-process seq)
    // BEFORE old consolidated events (large global seq) and the stale value
    // would win. recorded_at is present on EVERY event (record_event_append
    // stamps it once per dispatch) and is the same ts-primary order the merge
    // itself imposes, so it orders both populations correctly. sequence is the
    // tiebreak within one source (and the sole key when recorded_at is absent,
    // e.g. the pure-fold unit tests).
    let mut grouped: HashMap<(String, String), DeltaRows> =
        HashMap::new();
    for ev in events {
        let agg_name = ev.get("aggregate_name").as_str().unwrap_or("").to_string();
        let agg_id = ev.get("aggregate_id").as_str().unwrap_or("").to_string();
        let (field, value) = match ev.get("delta") {
            Value::Map(m) => (
                m.get("field").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                m.get("value").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            ),
            _ => continue,
        };
        let seq = match ev.get("sequence") {
            Value::Map(m) => m.get("value").and_then(|v| v.as_int()).unwrap_or(0),
            Value::Int(i) => *i,
            _ => 0,
        };
        let recorded_at = ev.get("recorded_at").as_str().unwrap_or("").to_string();
        if agg_name.is_empty() || field.is_empty() {
            continue;
        }
        grouped
            .entry((agg_name, agg_id))
            .or_default()
            .push((recorded_at, seq, field, value));
    }
    let mut out = HashMap::new();
    for (key, mut rows) in grouped {
        rows.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        let mut state = ReconstructedState::new();
        for (_, _, field, value) in rows {
            state.insert(field, value); // last write per field wins
        }
        out.insert(key, state);
    }
    out
}

/// Snapshot fall-forward : apply the watermark TAIL forward onto cached rows.
/// `rows` is a snapshot's cached projection (key `"<agg>::<id>::<field>"` ->
/// value) ; `tail` is every Event with sequence > the watermark (the
/// sequence-range seek's output). Folds the tail via `fold_event_log`
/// (sequence-ordered, last-write-wins) and overlays it onto `rows` — the tail is
/// newer, so it overrides. This is the trivial current-state projection : a
/// snapshot is never wrong, only stale, because the tail is always re-folded on
/// read. Custom-projection reducers are a later lift.
pub fn fold_forward_onto(
    mut rows: HashMap<String, String>,
    tail: &[&AggregateState],
) -> HashMap<String, String> {
    for ((agg, id), state) in fold_event_log(tail) {
        for (field, value) in state {
            rows.insert(format!("{}::{}::{}", agg, id, field), value);
        }
    }
    rows
}

/// One projection measurement : fold the COMPLETE Log and compare each
/// reconstructed field to the live store. The GAUGE of "state is derivable from
/// the Log" — the claim the whole substrate rests on.
pub struct ProjectionMeasurement {
    pub domain_instances: usize,
    pub infra: usize,
    pub orphan: usize,
    pub uncomparable: usize,
    pub total_fields: usize,
    /// (aggregate, id, field) -> (what the Log folds to, what the store holds)
    pub drift: DriftMap,
}

/// Measure once. `None` means the Log is empty (nothing to verify).
///
/// Lives HERE, beside the fold it measures, rather than in the CLI where it was
/// born — because it now has two callers : `storehouse verify-projection` (the
/// operator asking) and the Verification driver (the standing invariant asking on
/// a cadence). Two copies of a gauge is two gauges, and they drift.
pub fn measure(rt: &super::Runtime) -> Option<ProjectionMeasurement> {
    // The COMPLETE Log = consolidated event.heki + the unconsolidated shard tail.
    // A LIVE aggregate always has its newest event in the tail, so without it the
    // gauge reports a phantom lag-drift. The tail read is READ-ONLY.
    let consolidated = rt.all("Event");
    let tail = rt.unconsolidated_log_tail();
    if consolidated.is_empty() && tail.is_empty() {
        return None;
    }
    let mut events: Vec<&AggregateState> = consolidated;
    events.extend(tail.iter());
    let folded = fold_event_log(&events);

    let mut m = ProjectionMeasurement {
        domain_instances: 0,
        infra: 0,
        orphan: 0,
        uncomparable: 0,
        total_fields: 0,
        drift: HashMap::new(),
    };
    for ((agg_name, agg_id), recon) in &folded {
        // Infra mechanism is never part of the projection : its ids are
        // process-ephemeral, so the same predicate that stops the WRITE path also
        // excludes it from the GAUGE.
        if is_infra_mechanism(agg_name) {
            m.infra += 1;
            continue;
        }
        if agg_id.is_empty() || agg_id == "[0 items]" || agg_name == "[0 items]" {
            m.uncomparable += 1;
            continue;
        }
        m.domain_instances += 1;
        match rt.find(agg_name, agg_id) {
            None => m.orphan += 1, // Log has events but the live store has no record.
            Some(store) => {
                for (field, folded_val) in recon {
                    m.total_fields += 1;
                    // STRUCTURAL compare : decode the reconstructed delta JSON to a
                    // typed Value. `Value` equality is order-independent, so a Money
                    // {cents,currency} matches regardless of JSON key order — a
                    // string compare would false-drift on ordering.
                    let store_val = store.get(field);
                    let folded_value = super::value_from_json_str(folded_val);
                    if &folded_value != store_val {
                        m.drift.insert(
                            (agg_name.clone(), agg_id.clone(), field.clone()),
                            (folded_val.clone(), super::value_to_json_string(store_val)),
                        );
                    }
                }
            }
        }
    }
    Some(m)
}

/// Measure, then RE-measure to strip transient drift.
///
/// A real derivability failure (an unlogged write, a fold bug) recurs on every
/// pass ; a liveness race — the store moving between the tail read and the store
/// read of a continuously-dispatching aggregate — clears. You cannot atomically
/// snapshot two cross-process stores, so a single measurement CANNOT tell them
/// apart, but re-measurement can : persistent drift is the intersection.
///
/// This matters far more for the standing invariant than for the operator command
/// — a gauge that fires on every transient race trains everyone to ignore it.
/// Returns (persistent drift, transient count, the first measurement).
pub fn measure_persistent(
    rt: &super::Runtime,
    passes: usize,
) -> Option<(DriftMap, usize, ProjectionMeasurement)> {
    let first = measure(rt)?;
    let mut persistent = first.drift.clone();
    for _ in 0..passes {
        if persistent.is_empty() {
            break;
        }
        if let Some(again) = measure(rt) {
            persistent.retain(|k, _| again.drift.contains_key(k));
            // Keep the latest pass's values, so a report shows current numbers.
            for (k, v) in again.drift {
                if persistent.contains_key(&k) {
                    persistent.insert(k, v);
                }
            }
        }
    }
    let transient = first.drift.len().saturating_sub(persistent.len());
    Some((persistent, transient, first))
}

/// The runtime's own machinery — aggregates that are MECHANISM, not domain
/// intent : delivery bookkeeping (CascadeRun / Cascade), process-spawn
/// side-effects (Process), and liveness supervision (ProcessSentinel /
/// ProcessMacrophage). One list, two callers : the write path
/// (`record_event_append`) never SOURCES them and the proof
/// (`verify-projection`) never MEASURES them, so the gauge and the Log agree
/// by construction. Their ids are process-ephemeral, so fold(Log) cannot
/// reconstruct the live store — they are read models OF the Log, not intent.
///
/// OutboundEvent is DELIBERATELY NOT here : its delivery_id is stable
/// (`type::id::event::adapter`), so it IS event-sourced (persistence+ via its
/// hecksagon) — the outbox's Record/Claim/MarkDelivered history rides the Log,
/// and its Pending query is the read model of undelivered deliveries.
pub fn is_infra_mechanism(aggregate_type: &str) -> bool {
    matches!(
        aggregate_type,
        "CascadeRun" | "Cascade" | "Process" | "ProcessSentinel" | "ProcessMacrophage"
    )
}
