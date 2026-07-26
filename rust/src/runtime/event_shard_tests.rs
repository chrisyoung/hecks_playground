//! event_shard_tests — the per-process shard-writer suite : append/read
//! round-trips, seq ordering, opaque payload handling.
//!
//! Cask extracted VERBATIM from runtime/event_shard.rs (cask-runtime) ;
//! body dedented one level out of the old inline mod.
//!
//! [antibody-exempt: rust/src/runtime/event_shard_tests.rs — kernel-floor
//!  shard tests, relocated verbatim from event_shard.rs blanket.]

use super::event_shard::*;

fn rec(seq: u64, value: &str) -> ShardRecord {
    // The event payload is now opaque (the serialized Event aggregate).
    // Tests carry a `value` field inside it to assert round-trip fidelity.
    let mut event = serde_json::Map::new();
    event.insert("event_id".into(), serde_json::Value::String(format!("evt-{}", seq)));
    event.insert("aggregate_name".into(), serde_json::Value::String("Order".into()));
    event.insert("value".into(), serde_json::Value::String(value.into()));
    ShardRecord {
        shard: "p-42".into(),
        seq,
        ts: "2026-06-20T16:00:00Z".into(),
        event_id: format!("evt-{}", seq),
        event,
    }
}

// Read the test payload back out of the opaque event object.
fn val(r: &ShardRecord) -> &str {
    r.event.get("value").and_then(|v| v.as_str()).unwrap()
}

#[test]
fn round_trips_through_one_line() {
    let r = rec(1, "pending");
    let line = serialize_line(&r);
    assert!(!line.contains('\n'), "a record must encode to exactly one line");
    assert_eq!(parse_line(&line).unwrap(), r);
}

#[test]
fn newline_in_payload_stays_one_line() {
    // A field value with an embedded newline (e.g. Bash output) must not
    // break the one-record-per-line invariant — JSON escapes it.
    let r = rec(2, "line1\nline2\nline3");
    let line = serialize_line(&r);
    assert_eq!(line.matches('\n').count(), 0);
    assert_eq!(val(&parse_line(&line).unwrap()), "line1\nline2\nline3");
}

#[test]
fn large_payload_round_trips() {
    // Single-writer append is safe at ANY size — prove a >4KB record
    // (the size the multi-writer design could not handle) round-trips.
    let big = "x".repeat(8192);
    let r = rec(3, &big);
    let line = serialize_line(&r);
    assert!(line.len() > 8192);
    assert_eq!(val(&parse_line(&line).unwrap()), big);
}

#[test]
fn writer_then_read_from_zero() {
    let dir = std::env::temp_dir().join(format!("event_shard_test_{}", std::process::id()));
    let path = dir.join("a.shard");
    let _ = std::fs::remove_file(&path);
    let mut w = ShardWriter::open(&path).unwrap();
    w.append(&rec(1, "a")).unwrap();
    w.append(&rec(2, "b")).unwrap();
    let (records, off) = read_from(&path, 0).unwrap();
    assert_eq!(records.len(), 2);
    assert_eq!(val(&records[0]), "a");
    assert_eq!(val(&records[1]), "b");
    assert!(off > 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn resume_from_offset_reads_only_new_records() {
    let dir = std::env::temp_dir().join(format!("event_shard_resume_{}", std::process::id()));
    let path = dir.join("a.shard");
    let _ = std::fs::remove_file(&path);
    let mut w = ShardWriter::open(&path).unwrap();
    w.append(&rec(1, "a")).unwrap();
    let (first, off1) = read_from(&path, 0).unwrap();
    assert_eq!(first.len(), 1);
    // Append more AFTER the first read ; resuming from off1 sees only those.
    w.append(&rec(2, "b")).unwrap();
    w.append(&rec(3, "c")).unwrap();
    let (second, off2) = read_from(&path, off1).unwrap();
    assert_eq!(second.iter().map(val).collect::<Vec<_>>(), vec!["b", "c"]);
    assert!(off2 > off1);
    // Resuming from the tip yields nothing.
    let (none, off3) = read_from(&path, off2).unwrap();
    assert!(none.is_empty());
    assert_eq!(off3, off2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn partial_trailing_line_is_not_consumed() {
    // Simulate a write in flight : a line with no terminating newline.
    let dir = std::env::temp_dir().join(format!("event_shard_partial_{}", std::process::id()));
    let path = dir.join("a.shard");
    let _ = std::fs::remove_file(&path);
    std::fs::create_dir_all(&dir).unwrap();
    let complete = serialize_line(&rec(1, "a"));
    let partial = serialize_line(&rec(2, "b")); // no '\n' after this one
    std::fs::write(&path, format!("{}\n{}", complete, partial)).unwrap();
    let (records, off) = read_from(&path, 0).unwrap();
    // Only the complete line is consumed ; the partial waits.
    assert_eq!(records.len(), 1);
    assert_eq!(val(&records[0]), "a");
    assert_eq!(off as usize, complete.len() + 1); // up to and incl the '\n'
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_shard_reads_empty() {
    let path = std::env::temp_dir().join("event_shard_nonexistent_xyz.shard");
    let _ = std::fs::remove_file(&path);
    let (records, off) = read_from(&path, 0).unwrap();
    assert!(records.is_empty());
    assert_eq!(off, 0);
}
