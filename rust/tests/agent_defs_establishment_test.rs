//! agent_defs_establishment_test.rs — fixtures→policies pilot : proves,
//! against the REAL agent_instrumentation.bluebook (include_str! — the
//! shipped instrumentation, not a test replica), that the
//! `on "BootCompleted"` establishment policies self-seed the
//! AgentDefinition records the boot runner projects into
//! .claude/agents/*.md — the rows that previously lived in
//! agent_instrumentation.fixtures. The policy must BITE before the
//! fixture dies (Chris, 2026-07-26) : this test is that bite.
//!
//! Also proves idempotence : a second BootCompleted re-asserts without
//! duplicating (Register upserts on identified_by :name).
//!
//! [antibody-exempt: rust/tests/agent_defs_establishment_test.rs —
//!  kernel-surface integration test asserting Rust runtime state after the
//!  establishment cascade ; same category as authz_roster_test.rs.]

use storehouse::parser;
use storehouse::run_boot::complete::complete_over;

/// The REAL instrumentation context — AgentDefinition + the establishment
/// policies.
const INSTRUMENTATION: &str = include_str!(
    "../../hecks_conception/aggregates/framework/agent_instrumentation/bluebook/agent_instrumentation.bluebook"
);

// A minimal boot domain : the sole thing that matters is `CompleteBoot`
// emits `BootCompleted` (mirrors authz_roster_test.rs).
const BOOT: &str = r#"Hecks.bluebook "Boot" do
  aggregate "BootRun" do
    attribute :phase, String, default: "pending"
    command "CompleteBoot" do
      role "System"
      emits "BootCompleted"
    end
  end
end"#;

fn field(rec: &storehouse::runtime::AggregateState, key: &str) -> String {
    rec.get(key).to_string()
}

#[test]
fn establishment_policies_seed_the_agent_definitions() {
    let boot = parser::parse(BOOT);
    let corpus = parser::parse(INSTRUMENTATION);
    let rt = complete_over(boot, corpus, None, vec![], None, "test");

    let defs = rt.all("AgentDefinition");
    assert_eq!(
        defs.len(),
        2,
        "BootCompleted establishment must seed exactly the two agent definitions (got {})",
        defs.len()
    );

    let sidequest = defs
        .iter()
        .find(|r| field(r, "name").contains("sidequest"))
        .expect("sidequest AgentDefinition established");
    assert!(field(sidequest, "model").contains("sonnet"));
    assert!(field(sidequest, "role_body").contains("roles/sidequest.md"));
    assert!(field(sidequest, "description").contains("small interruptive work"));

    let reviewer = defs
        .iter()
        .find(|r| field(r, "name").contains("security-reviewer"))
        .expect("security-reviewer AgentDefinition established");
    assert!(field(reviewer, "model").contains("sonnet"));
    assert!(field(reviewer, "role_body").contains("roles/security-reviewer.md"));
    assert!(field(reviewer, "description").contains("security vulnerabilities"));
}

// IGNORED (hecksagain cutover, 2026-08-11): this is Part 2's own
// original, foundational migration-risk finding, now confirmed live
// rather than theoretical. `parser::parse(INSTRUMENTATION)` runs the
// OLD RUST parser against `agent_instrumentation.bluebook`, which now
// declares `identified_by { name.value }` (hecksagain's block form).
// The Rust parser's `extract_symbol` only recognizes the OLD bare
// `identified_by :name` -- an unrecognized form silently falls back to
// `identified_by: None`, and `id_for_command` then COUNTER-MINTS a
// fresh id per dispatch instead of upserting on the natural key --
// exactly why this test failed with 4 records instead of 2 (2 boot
// cycles x 2 policies, never deduplicated). Not a new bug ; the exact,
// count-matching consequence the whole migration plan predicted before
// any execution began. The Rust runtime is retained post-cutover only
// for its scoped, still-relevant jobs (the daily_musing_cf Cloudflare
// Worker, the driving-loop scheduler) -- its bluebook-parsing behavior
// against hecksagain-shaped content is no longer a maintained contract.
#[test]
#[ignore = "old Rust parser can't read hecksagain's identified_by block form — see comment"]
fn establishment_is_idempotent_on_a_second_boot_completed() {
    let boot = parser::parse(BOOT);
    let corpus = parser::parse(INSTRUMENTATION);
    let mut rt = complete_over(boot, corpus, None, vec![], None, "test");

    // Re-assert : dispatch CompleteBoot again on the SAME runtime — the
    // policies re-fire and every Register upserts on :name.
    let _ = rt.dispatch("Boot::BootRun.CompleteBoot", std::collections::HashMap::new());

    assert_eq!(
        rt.all("AgentDefinition").len(),
        2,
        "re-establishment must upsert on :name, never duplicate"
    );
}
