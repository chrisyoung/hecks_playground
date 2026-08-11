//! causation_e2e_tests — the END-TO-END causation proof : a real policy
//! cascade through dispatch() (heki-backed runtime from a temp dir with
//! the REAL Event + CascadeRun chapters, event sourcing ON) stamps
//! causation_id on the cascaded command's recorded event == the trigger's
//! event id — the durable chain, append → run → pump → stamp. The
//! unit-grain walk tests stay in causation_trace_tests.rs.
//!
//! Cask extracted VERBATIM from runtime/causation_trace_tests.rs
//! (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/causation_e2e_tests.rs — kernel-floor
//!  lineage e2e test, relocated verbatim from causation_trace_tests.rs
//!  blanket.]

use super::*;

// END-TO-END : a real cascade through dispatch() stamps causation_id on the
// cascaded command's recorded events. Boots a heki-backed runtime from a temp
// dir holding the REAL Event + CascadeRun chapters + a minimal policy cascade
// (Trigger.Fire -> Fired -> Target.Land), event sourcing ON, then reads the
// Event store and asserts Target.Land's event carries causation_id ==
// Trigger.Fire's event id. Proves the DURABLE chain end to end :
// record_event_append notes the trigger -> record_cascade_run persists it on
// the run -> the pump primes the map -> the cascaded command's
// record_event_append stamps it. (Default-heki Event upserts each Append into
// the Event store, so no AppendLog/merge wiring is needed to observe it.)
#[test]
fn cascade_stamps_causation_end_to_end() {
    use std::collections::HashMap;
    // Post-extraction the engine no longer sits beside hecks_conception, so
    // resolve the live framework chapters via HECKS_CONCEPTION_DIR (the gate
    // sets it), falling back to the standard sibling checkout (storehouse
    // beside hecks) for a bare local run.
    let conception = std::env::var("HECKS_CONCEPTION_DIR").unwrap_or_else(|_| {
        // In-tree the engine sits at <hecks>/rust (CI's shape) ; post-
        // extraction it sits beside the hecks repo.
        let in_tree = format!("{}/../hecks_conception", env!("CARGO_MANIFEST_DIR"));
        if std::path::Path::new(&in_tree).is_dir() {
            in_tree
        } else {
            format!("{}/../../hecks/hecks_conception", env!("CARGO_MANIFEST_DIR"))
        }
    });
    let fw = format!("{}/aggregates/framework", conception);
    let dir = std::env::temp_dir().join(format!("caus_e2e_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(
        format!("{}/event_sourcing/bluebook/event_sourcing.bluebook", fw),
        dir.join("event_sourcing.bluebook"),
    )
    .unwrap();
    std::fs::copy(
        format!("{}/cascade/bluebook/cascade_run.bluebook", fw),
        dir.join("cascade_run.bluebook"),
    )
    .unwrap();
    let cascade_bb = r#"Hecks.bluebook "CausTest" do
core
aggregate "Trigger" do
identified_by :name
attribute :name,  Name
attribute :fired, Fired, default: "no"
value_object "Name" do
  attribute :value, String
end
value_object "Fired" do
  attribute :value, String
end
command "Fire" do
  role "System"
  goal "Fire so the policy cascades Target.Land"
  attribute :name, Name
  then_set :fired, to: "yes"
  emits "Fired"
end
end
aggregate "Target" do
identified_by :name
attribute :name,   Name
attribute :landed, Landed, default: "no"
value_object "Name" do
  attribute :value, String
end
value_object "Landed" do
  attribute :value, String
end
command "Land" do
  role "System"
  goal "Record the cascaded landing"
  attribute :name, Name
  then_set :landed, to: "yes"
  emits "Landed"
end
end
policy "LandOnFired" do
on "Trigger.Fired"
trigger "CausTest::Target.Land"
with "name", "t1"
end
end
"#;
    std::fs::write(dir.join("caustest.bluebook"), cascade_bb).unwrap();

    let domain = crate::corpus_loader::load_combined_domain(dir.to_str().unwrap());
    let data = dir.join("data").to_string_lossy().into_owned();
    std::env::set_var("HECKS_EVENT_SOURCING", "1");
    let mut rt = Runtime::boot_with_data_dir(domain, Some(data));
    let mut attrs = HashMap::new();
    attrs.insert("name".to_string(), Value::Str("t1".to_string()));
    rt.dispatch("CausTest::Trigger.Fire", attrs).expect("Fire dispatch");
    std::env::remove_var("HECKS_EVENT_SOURCING");

    let events = rt.all_qualified(Some("EventSourcing"), "Event");
    let agg = |s: &AggregateState| s.get("aggregate_name").to_string();
    let trigger_ids: std::collections::HashSet<String> = events
        .iter()
        .filter(|s| agg(s) == "Trigger")
        .map(|s| s.id.clone())
        .collect();
    let targets: Vec<&AggregateState> = events
        .iter()
        .filter(|s| agg(s) == "Target")
        .copied()
        .collect();
    assert!(!trigger_ids.is_empty(), "Trigger.Fire recorded at least one event");
    assert!(!targets.is_empty(), "the cascade recorded a Target.Land event");
    for t in &targets {
        let caus = t.get("causation_id").to_string();
        assert!(!caus.is_empty(), "cascaded Target event must carry causation_id");
        assert!(
            trigger_ids.contains(&caus),
            "Target.causation ({}) must equal a Trigger event id {:?}",
            caus,
            trigger_ids
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}
