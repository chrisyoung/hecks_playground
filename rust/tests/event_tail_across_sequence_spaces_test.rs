//! THE TWO SEQUENCE SPACES — the read must not lose the unconsolidated tail.
//!
//! The Log numbers events in two different spaces:
//!   * CONSOLIDATED records carry a GLOBAL sequence, assigned by the merge.
//!   * UNCONSOLIDATED records sit in this process's shard carrying a PER-PROCESS
//!     sequence that starts again at 1 on every boot.
//!
//! A Snapshot watermark lives in the global space. `ReadForward` used to fold the
//! tail through `sequence.value > watermark` — comparing the two spaces — so once a
//! snapshot existed at, say, 306891, a freshly written event stamped `sequence: 1`
//! could never clear it and was SILENTLY DROPPED from the derived state. Measured on
//! the live store: the raw Log held ten Vault instances while the derived
//! current_state projection held ZERO. That is Stage 4's whole claim — "state is
//! DERIVED from the Log" — quietly not deriving.
//!
//! `event_snapshot_capture_test` could not catch it: there, every event is in the
//! tail AND the watermark came from that same per-process space, so the comparison
//! happened to be self-consistent. The bug needs BOTH spaces present at once — a
//! consolidated Log plus a fresh process appending to a shard. That is exactly a
//! long-lived realm, and exactly what no test built. This builds it.
//!
//! Its own test binary: it tunes `HECKS_SNAPSHOT_EVERY`, and env is process-global.

mod realm_fixture;

use std::collections::HashMap;
use storehouse::runtime::{event_log, Runtime, Value};

const COUNTER: &str = r#"Hecks.bluebook "Tally" do
  core
  aggregate "Counter" do
    identified_by :name
    attribute :name, Name
    attribute :hits, Hits, default: "0"
    value_object "Name" do
      attribute :value, String
    end
    value_object "Hits" do
      attribute :value, String
    end
    command "Bump" do
      role "System"
      attribute :name, Name
      attribute :hits, Hits
      then_set :hits, to: :hits
      emits "Bumped"
    end
  end
end
"#;

const COUNTER_HEX: &str = r#"Hecks.hecksagon "Tally" do
  Tally::Counter.persisted_by("Heki")
  Tally::Counter.event_sourced
end
"#;

fn bump(rt: &mut Runtime, name: &str, hits: i64) {
    let mut attrs = HashMap::new();
    attrs.insert("name".to_string(), Value::Str(name.to_string()));
    attrs.insert("hits".to_string(), Value::Str(hits.to_string()));
    rt.dispatch("Tally::Counter.Bump", attrs).expect("Bump dispatches");
}

fn read_forward(rt: &Runtime) -> Vec<(String, String)> {
    let mut params = HashMap::new();
    params.insert("projection_name".to_string(), "current_state".to_string());
    rt.resolve_query("ReadForward", &params)
        .get("state")
        .and_then(|s| s.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|r| {
                    Some((
                        r.get("key")?.as_str()?.to_string(),
                        r.get("value")?.as_str().unwrap_or_default().to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn watermark(rt: &mut Runtime) -> i64 {
    match rt.framework_mut().find("Snapshot", "current_state").map(|s| s.get("watermark").clone()) {
        Some(Value::Int(i)) => i,
        Some(Value::Map(m)) => m.get("value").and_then(|v| v.as_int()).unwrap_or(0),
        _ => 0,
    }
}

#[test]
fn a_high_global_watermark_does_not_hide_the_low_sequenced_tail() {
    // Capture on every event, so a snapshot exists as early as possible.
    std::env::set_var("HECKS_SNAPSHOT_EVERY", "1");
    let (mut rt, root) =
        realm_fixture::boot_realm("es_two_sequence_spaces", "tally", COUNTER, COUNTER_HEX);

    // Stand up a realm that has ALREADY consolidated a long way: push the global
    // sequence sidecar out to 5000. This is what any long-lived realm looks like,
    // and it is the half no in-process test ever had.
    let global = event_log::global_path(
        root.join("data").to_str().expect("utf8 data dir"),
        Some("EventSourcing"),
    );
    std::fs::create_dir_all(std::path::Path::new(&global).parent().unwrap()).unwrap();
    event_log::write_next_seq(&global, 5000);

    // Now dispatch. These events go to THIS process's shard with per-process
    // sequences 1, 2, 3… — far below the consolidated head of 4999.
    bump(&mut rt, "c1", 7);
    bump(&mut rt, "c1", 8);

    // The snapshot's watermark is in the GLOBAL space and is high.
    let wm = watermark(&mut rt);
    assert!(
        wm >= 4999,
        "the watermark tracks the CONSOLIDATED head, which this realm pushed to 4999 \
         (got {wm}) — if this is small, the watermark is being drawn from the \
         per-process space and the whole premise of this test is gone",
    );

    // THE PROPERTY: the freshly written, low-sequenced tail is still in the derived
    // state. Before the fix this returned nothing for Counter — every event sat at
    // sequence 1..2, the watermark said 4999, and the reader filtered them all away.
    let rows = read_forward(&rt);
    let hits: Vec<&(String, String)> =
        rows.iter().filter(|(k, _)| k == "Counter::c1::hits").collect();
    assert!(
        !hits.is_empty(),
        "the unconsolidated tail must fold into the derived state even though its \
         per-process sequences sit far below the global watermark — got rows {rows:?}",
    );
    assert_eq!(
        hits[0].1, "\"8\"",
        "and it must fold FORWARD to the latest value, not the first",
    );

    // And it keeps holding as the Log moves on past the capture.
    bump(&mut rt, "c1", 9);
    let rows = read_forward(&rt);
    let hits: Vec<&(String, String)> =
        rows.iter().filter(|(k, _)| k == "Counter::c1::hits").collect();
    assert_eq!(
        hits.first().map(|(_, v)| v.as_str()),
        Some("\"9\""),
        "a later append must still reach the read — rows {rows:?}",
    );

    std::env::remove_var("HECKS_SNAPSHOT_EVERY");
    let _ = std::fs::remove_dir_all(&root);
}
