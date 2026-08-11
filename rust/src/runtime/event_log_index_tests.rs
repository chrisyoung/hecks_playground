//! event_log_index_tests — the sidecar-index suite : build/read
//! round-trips, stale-index detection, offset lookup.
//!
//! Cask extracted VERBATIM from runtime/event_log_index.rs (cask-runtime) ;
//! body dedented one level out of the old inline mod.
//!
//! [antibody-exempt: rust/src/runtime/event_log_index_tests.rs —
//!  kernel-floor index tests, relocated verbatim from event_log_index.rs
//!  blanket.]

use super::event_log_index::*;
use super::event_log_index::write_marker;
use std::io::Write;

fn tmp(name: &str) -> String {
    let p = std::env::temp_dir()
        .join(format!("event_log_index_{}_{}/event.log", std::process::id(), name));
    let _ = std::fs::remove_dir_all(p.parent().unwrap());
    let _ = std::fs::create_dir_all(p.parent().unwrap());
    p.to_string_lossy().into_owned()
}

// Append two lines through the same offset bookkeeping append_records uses,
// then assert the index resolves each name to the right offsets AND that
// seeking to those offsets reads the matching lines back.
fn write_log_line(global: &str, l0: u64, name: &str, id: &str) -> (u64, String) {
    let line = format!("{{\"event_id\":\"{}\",\"aggregate_name\":\"{}\"}}", id, name);
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(global).unwrap();
    f.write_all(line.as_bytes()).unwrap();
    f.write_all(b"\n").unwrap();
    (l0, line)
}

#[test]
fn index_resolves_offsets_and_marker_gates_completeness() {
    let g = tmp("basic");
    // line 0 : Order, line 1 : Pizza, line 2 : Order
    let (o0, l0) = write_log_line(&g, 0, "Order", "e0");
    let len0 = l0.len() as u64 + 1;
    let (o1, l1) = write_log_line(&g, len0, "Pizza", "e1");
    let len1 = len0 + l1.len() as u64 + 1;
    let (o2, l2) = write_log_line(&g, len1, "Order", "e2");
    let total = len1 + l2.len() as u64 + 1;
    record_appends(&g, 0, &[(o0, "Order".into(), 1), (o1, "Pizza".into(), 2), (o2, "Order".into(), 3)], total);

    let offs = complete_offsets_for(&g, "Order").expect("index complete");
    assert_eq!(offs, vec![0, len1]);
    assert_eq!(complete_offsets_for(&g, "Pizza").unwrap(), vec![len0]);
}

#[test]
fn incomplete_marker_returns_none_for_full_scan_fallback() {
    let g = tmp("incomplete");
    let (o0, l0) = write_log_line(&g, 0, "Order", "e0");
    // Marker deliberately behind the log (simulating a crash before commit).
    write_marker(&g, 0);
    let _ = (o0, l0);
    assert!(complete_offsets_for(&g, "Order").is_none());
}

#[test]
fn rebuild_heals_a_missing_index_on_next_append() {
    let g = tmp("heal");
    // Two lines exist in the log but the index was never built.
    let (_o0, l0) = write_log_line(&g, 0, "Order", "e0");
    let len0 = l0.len() as u64 + 1;
    let (o1, l1) = write_log_line(&g, len0, "Order", "e1");
    let total = len0 + l1.len() as u64 + 1;
    // record_appends sees no index (marker 0 != l0=len0) and rebuilds [0..len0]
    // before appending the new entry — so BOTH events are indexed.
    record_appends(&g, len0, &[(o1, "Order".into(), 2)], total);
    assert_eq!(complete_offsets_for(&g, "Order").unwrap(), vec![0, len0]);
}

#[test]
fn seq_range_resolves_inclusive_bounds_and_open_ended() {
    let g = tmp("seqrange");
    let (o0, l0) = write_log_line(&g, 0, "Order", "e0");
    let len0 = l0.len() as u64 + 1;
    let (o1, l1) = write_log_line(&g, len0, "Order", "e1");
    let len1 = len0 + l1.len() as u64 + 1;
    let (o2, l2) = write_log_line(&g, len1, "Order", "e2");
    let total = len1 + l2.len() as u64 + 1;
    // sequences 5, 6, 7 at offsets 0, len0, len1.
    record_appends(&g, 0, &[(o0, "Order".into(), 5), (o1, "Order".into(), 6), (o2, "Order".into(), 7)], total);
    // inclusive [6,7]
    assert_eq!(complete_offsets_in_seq_range(&g, Some(6), Some(7)).unwrap(), vec![len0, len1]);
    // open-ended low (..,6]
    assert_eq!(complete_offsets_in_seq_range(&g, None, Some(6)).unwrap(), vec![0, len0]);
    // open-ended high [6,..)
    assert_eq!(complete_offsets_in_seq_range(&g, Some(6), None).unwrap(), vec![len0, len1]);
    // above all -> empty
    assert!(complete_offsets_in_seq_range(&g, Some(8), None).unwrap().is_empty());
}

#[test]
fn seq_range_handles_gaps() {
    let g = tmp("seqgap");
    let (o0, l0) = write_log_line(&g, 0, "Order", "e0");
    let len0 = l0.len() as u64 + 1;
    let (o1, l1) = write_log_line(&g, len0, "Order", "e1");
    let total = len0 + l1.len() as u64 + 1;
    // sequences 10 and 20 (a gap between).
    record_appends(&g, 0, &[(o0, "Order".into(), 10), (o1, "Order".into(), 20)], total);
    assert!(complete_offsets_in_seq_range(&g, Some(11), Some(19)).unwrap().is_empty());
    assert_eq!(complete_offsets_in_seq_range(&g, Some(10), Some(20)).unwrap(), vec![0, len0]);
    assert_eq!(complete_offsets_in_seq_range(&g, Some(15), None).unwrap(), vec![len0]);
}

#[test]
fn seq_range_incomplete_marker_returns_none() {
    let g = tmp("seqincomplete");
    let (o0, l0) = write_log_line(&g, 0, "Order", "e0");
    let _ = (o0, l0);
    write_marker(&g, 0); // marker behind the log
    assert!(complete_offsets_in_seq_range(&g, Some(1), None).is_none());
}

#[test]
fn migration_ignores_legacy_2col_idx_and_rebuilds_idx2() {
    let g = tmp("migrate");
    let lines = [
        "{\"event_id\":\"e0\",\"aggregate_name\":\"Order\",\"sequence\":{\"value\":1}}",
        "{\"event_id\":\"e1\",\"aggregate_name\":\"Pizza\",\"sequence\":{\"value\":2}}",
        "{\"event_id\":\"e2\",\"aggregate_name\":\"Order\",\"sequence\":{\"value\":3}}",
    ];
    let len0 = lines[0].len() as u64 + 1;
    let len1 = len0 + lines[1].len() as u64 + 1;
    let total = len1 + lines[2].len() as u64 + 1;
    std::fs::write(&g, format!("{}\n{}\n{}\n", lines[0], lines[1], lines[2])).unwrap();
    // Pre-seed a STALE legacy 2-col .idx + marker (the pre-migration state).
    std::fs::write(format!("{}.idx", g), "0\tOrder\n").unwrap();
    std::fs::write(format!("{}.idx.len", g), total.to_string()).unwrap();
    // Append e2 : new code finds no .idx2 -> rebuilds [0..len1] in 3-col, adds e2.
    record_appends(&g, len1, &[(len1, "Order".into(), 3)], total);
    // Name seek works for all (rebuild read names from the log).
    assert_eq!(complete_offsets_for(&g, "Order").unwrap(), vec![0, len1]);
    assert_eq!(complete_offsets_for(&g, "Pizza").unwrap(), vec![len0]);
    // Sequence-range seek works (proves the 3rd column was rebuilt from the log).
    assert_eq!(complete_offsets_in_seq_range(&g, Some(2), Some(3)).unwrap(), vec![len0, len1]);
    // The legacy 2-col .idx was never read — still our stale stub.
    assert_eq!(std::fs::read_to_string(format!("{}.idx", g)).unwrap(), "0\tOrder\n");
}
