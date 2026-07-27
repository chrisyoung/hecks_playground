//! daily_musing_establishment_test.rs — fixtures→policies site 5 (the
//! daily-musing Worker's demo seeds) : proves, against the REAL
//! daily_musing.bluebook (include_str! — the shipped blog domain, not a
//! test replica), that the `on "BootCompleted"` establishment policies
//! self-seed the two published BlogEntry records that previously lived in
//! daily_musing.fixtures (embedded into the Worker and seeded straight
//! onto AggregateState). The policy must BITE before the fixture dies
//! (Chris, 2026-07-26) : this test is that bite.
//!
//! The Worker rides the same seam : its generated lib.rs (wasm_worker
//! shape) dispatches CompleteBoot on the merged per-request runtime, so
//! these policies fire there exactly as complete_over fires them here.
//!
//! [antibody-exempt: rust/tests/daily_musing_establishment_test.rs —
//!  kernel-surface integration test asserting Rust runtime state after the
//!  establishment cascade ; same category as agent_defs_establishment_test.rs.]

use storehouse::parser;
use storehouse::run_boot::complete::complete_over;
use storehouse::runtime::Value;

/// The REAL blog domain — BlogEntry + the establishment policies.
const DAILY_MUSING: &str =
    include_str!("../../deployments/daily_musing_cf/domain/daily_musing.bluebook");

// A minimal boot domain : the sole thing that matters is `CompleteBoot`
// emits `BootCompleted` (mirrors agent_defs_establishment_test.rs — and
// the Worker's embedded boot.bluebook).
const BOOT: &str = r#"Hecks.bluebook "Boot" do
  aggregate "BootRun" do
    attribute :phase, String, default: "pending"
    command "CompleteBoot" do
      role "System"
      emits "BootCompleted"
    end
  end
end"#;

fn bare(v: &Value) -> String {
    match v {
        Value::Str(s) => s.clone(),
        Value::Map(m) => m.get("value").map(bare).unwrap_or_default(),
        other => other.to_string(),
    }
}

#[test]
fn establishment_policies_seed_the_two_published_musings() {
    let boot = parser::parse(BOOT);
    let corpus = parser::parse(DAILY_MUSING);
    let rt = complete_over(boot, corpus, None, vec![], None, "test");

    let entries = rt.all("BlogEntry");
    assert_eq!(
        entries.len(),
        2,
        "BootCompleted establishment must seed exactly the two demo musings (got {})",
        entries.len()
    );

    let welcome = entries
        .iter()
        .find(|r| bare(r.get("title")) == "Welcome to the Daily Musing")
        .expect("welcome musing established");
    assert_eq!(bare(welcome.get("author")), "Miette");
    assert_eq!(bare(welcome.get("published_at")), "2026-05-23T08:00:00Z");
    assert!(bare(welcome.get("body")).contains("compiled to WebAssembly"));
    // The DailyMusing query filters `where status: "published"` — the Post
    // lifecycle transition must land every seeded entry there, exactly as
    // the fixture rows set status explicitly.
    assert_eq!(bare(welcome.get("status")), "published");

    let morning = entries
        .iter()
        .find(|r| bare(r.get("title")) == "Morning Light")
        .expect("morning-light musing established");
    assert_eq!(bare(morning.get("author")), "Chris");
    assert_eq!(bare(morning.get("published_at")), "2026-05-23T09:30:00Z");
    assert_eq!(bare(morning.get("status")), "published");
}

#[test]
fn establishment_is_idempotent_on_a_second_boot_completed() {
    let boot = parser::parse(BOOT);
    let corpus = parser::parse(DAILY_MUSING);
    let mut rt = complete_over(boot, corpus, None, vec![], None, "test");

    let _ = rt.dispatch("Boot::BootRun.CompleteBoot", std::collections::HashMap::new());

    assert_eq!(
        rt.all("BlogEntry").len(),
        2,
        "re-establishment must upsert on :title, never duplicate"
    );
}
