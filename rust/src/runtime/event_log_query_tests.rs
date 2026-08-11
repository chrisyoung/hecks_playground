//! event_log_query_tests — the pushdown-query suite : eq matching, kwarg
//! resolution from attrs, pushable-clause gating, index fast path vs
//! stream-path oracle, seq_range seek narrowing.
//!
//! Cask extracted VERBATIM from runtime/event_log_query.rs (cask-runtime) ;
//! body dedented one level out of the old inline mod.
//!
//! [antibody-exempt: rust/src/runtime/event_log_query_tests.rs —
//!  kernel-floor event-log tests, relocated verbatim from
//!  event_log_query.rs blanket.]

use super::event_log_query::load_filtered;
use super::AggregateState;
use crate::ir::{WhereClause, WhereOp};
use std::collections::HashMap;
use std::io::Write;

fn write_log(name: &str, lines: &[&str]) -> String {
    let path = std::env::temp_dir()
        .join(format!("event_log_query_{}_{}/event.log", std::process::id(), name));
    let _ = std::fs::create_dir_all(path.parent().unwrap());
    let mut f = std::fs::File::create(&path).unwrap();
    for l in lines {
        writeln!(f, "{}", l).unwrap();
    }
    path.to_string_lossy().into_owned()
}

fn clause(field: &str, op: WhereOp, value: &str) -> WhereClause {
    WhereClause { field: field.into(), op, value: value.into() }
}

const LOG: &[&str] = &[
    r#"{"event_id":"e1","aggregate_name":"Order","aggregate_id":"o1"}"#,
    r#"{"event_id":"e2","aggregate_name":"Pizza","aggregate_id":"p1"}"#,
    r#"{"event_id":"e3","aggregate_name":"Order","aggregate_id":"o2"}"#,
];

#[test]
fn eq_on_aggregate_name_returns_only_matches() {
    let path = write_log("replay", LOG);
    let attrs = HashMap::new();
    let out = load_filtered(&path, &[clause("aggregate_name", WhereOp::Eq, "Order")], &attrs)
        .unwrap();
    let mut ids: Vec<String> = out.iter().map(|s| s.id.clone()).collect();
    ids.sort();
    assert_eq!(ids, vec!["e1".to_string(), "e3".to_string()]);
}

#[test]
fn kwarg_target_resolves_from_attrs() {
    let path = write_log("kwarg", LOG);
    let mut attrs = HashMap::new();
    attrs.insert("aggregate_name".to_string(), "Pizza".to_string());
    let out =
        load_filtered(&path, &[clause("aggregate_name", WhereOp::Eq, ":aggregate_name")], &attrs)
            .unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].id, "e2");
}

#[test]
fn no_pushable_clause_returns_none() {
    let path = write_log("nopush", LOG);
    let attrs = HashMap::new();
    assert!(load_filtered(&path, &[clause("sequence", WhereOp::Gt, "1")], &attrs).is_none());
    assert!(load_filtered(&path, &[], &attrs).is_none());
}

#[test]
fn missing_file_with_pushable_clause_is_empty_not_none() {
    let attrs = HashMap::new();
    let out = load_filtered(
        "/tmp/does_not_exist_event_log_query.log",
        &[clause("aggregate_name", WhereOp::Eq, "Order")],
        &attrs,
    );
    assert!(matches!(out, Some(ref v) if v.is_empty()));
}

// The Phase-3 index fast path and the Phase-2 stream fallback must return
// IDENTICAL results. Build the Log through the real append_records (which
// maintains the index + commit marker), query once via the index, then drop
// the marker to force the stream path — same ids both ways.
#[test]
fn index_fast_path_matches_stream_path() {
    use super::event_shard::ShardRecord;
    let dir = std::env::temp_dir().join(format!("elq_idx_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let global = dir.join("event.log").to_string_lossy().into_owned();
    let mk = |id: &str, name: &str| {
        let mut event = serde_json::Map::new();
        event.insert("event_id".into(), serde_json::Value::String(id.into()));
        event.insert("aggregate_name".into(), serde_json::Value::String(name.into()));
        ShardRecord { shard: "s".into(), seq: 0, ts: "t".into(), event_id: id.into(), event }
    };
    super::event_log::append_records(
        &global,
        &[mk("e1", "Order"), mk("e2", "Pizza"), mk("e3", "Order")],
    )
    .unwrap();
    let attrs = HashMap::new();
    let wheres = [clause("aggregate_name", WhereOp::Eq, "Order")];

    let mut via_idx: Vec<String> = load_filtered(&global, &wheres, &attrs)
        .unwrap()
        .iter()
        .map(|s| s.id.clone())
        .collect();
    via_idx.sort();
    assert_eq!(via_idx, vec!["e1".to_string(), "e3".to_string()]);

    // Drop the commit marker — the reader must fall back to the stream scan
    // and produce the same ids.
    let _ = std::fs::remove_file(format!("{}.idx2.len", global));
    let mut via_stream: Vec<String> = load_filtered(&global, &wheres, &attrs)
        .unwrap()
        .iter()
        .map(|s| s.id.clone())
        .collect();
    via_stream.sort();
    assert_eq!(via_stream, via_idx);
}

// A pure `sequence.value > N` must be PUSHABLE (reachable for Snapshot
// fall-forward), the SEEK must narrow by range, and after the oracle the
// seek and the stream fallback must agree. (The stream path returns a
// superset for a range-only query — line_matches enforces only Eq — so
// parity is asserted through the oracle, the final authority.)
#[test]
fn seq_range_seek_is_pushable_narrows_and_matches_oracle() {
    use super::event_shard::ShardRecord;
    let dir = std::env::temp_dir().join(format!("elq_seq_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let global = dir.join("event.log").to_string_lossy().into_owned();
    let mk = |id: &str, name: &str| {
        let mut event = serde_json::Map::new();
        event.insert("event_id".into(), serde_json::Value::String(id.into()));
        event.insert("aggregate_name".into(), serde_json::Value::String(name.into()));
        ShardRecord { shard: "s".into(), seq: 0, ts: "t".into(), event_id: id.into(), event }
    };
    // append_records assigns sequences 1, 2, 3.
    super::event_log::append_records(
        &global,
        &[mk("e1", "Order"), mk("e2", "Pizza"), mk("e3", "Order")],
    )
    .unwrap();
    let attrs = HashMap::new();
    let wheres = [clause("sequence.value", WhereOp::Gt, "1")];

    // Pushable + the seek narrows by range : offsets are exactly seq 2,3.
    let seek = load_filtered(&global, &wheres, &attrs).expect("pure seq-range is pushable");
    let mut seek_ids: Vec<String> = seek.iter().map(|s| s.id.clone()).collect();
    seek_ids.sort();
    assert_eq!(seek_ids, vec!["e2".to_string(), "e3".to_string()], "gt 1 excludes boundary e1");

    // Oracle-parity : seek vs stream fallback, after the final authority.
    let oracle = |cands: &[AggregateState]| {
        let mut ids: Vec<String> = cands
            .iter()
            .filter(|s| wheres.iter().all(|w| super::where_matches(s, w, &attrs)))
            .map(|s| s.id.clone())
            .collect();
        ids.sort();
        ids
    };
    let _ = std::fs::remove_file(format!("{}.idx2.len", global));
    let stream = load_filtered(&global, &wheres, &attrs).unwrap();
    assert_eq!(oracle(&seek), oracle(&stream), "oracle(seek) == oracle(stream)");
    assert_eq!(oracle(&seek), vec!["e2".to_string(), "e3".to_string()]);
}
