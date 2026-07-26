//! projection_measure — the derivability GAUGE : ProjectionMeasurement
//! (fold the COMPLETE Log, compare each reconstructed field to the live
//! store), measure / measure_persistent (one gauge, two callers — the
//! operator's verify-projection and the standing Verification driver),
//! and the is_infra_mechanism filter. The fold itself stays in
//! projection_fold.rs ; old paths hold via re-exports.
//!
//! Cask extracted VERBATIM from runtime/projection_fold.rs (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/projection_measure.rs — kernel-floor
//!  derivability gauge, relocated verbatim from projection_fold.rs blanket.]

use super::projection_fold::{fold_event_log, DriftMap};
use super::AggregateState;
use std::collections::HashMap;

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
