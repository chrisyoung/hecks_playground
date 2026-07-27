//! subcommand_registry_establishment_test.rs — fixtures→policies site 6
//! (the CLI subcommand catalog) : proves, against the REAL
//! subcommand.bluebook (include_str! — the shipped registry domain, not a
//! test replica), that the 30 `on "BootCompleted"` establishment policies
//! self-seed the Subcommand rows that previously lived in
//! subcommand.fixtures (routed through bin/seed-subcommand-registry's
//! per-row cold dispatches). The policy must BITE before the fixture dies
//! (Chris, 2026-07-26) : this test is that bite.
//!
//! [antibody-exempt: rust/tests/subcommand_registry_establishment_test.rs —
//!  kernel-surface integration test asserting Rust runtime state after the
//!  establishment cascade ; same category as agent_defs_establishment_test.rs.]

use storehouse::parser;
use storehouse::run_boot::complete::complete_over;
use storehouse::runtime::Value;

/// The REAL registry domain — Subcommand + the establishment policies.
const SUBCOMMAND: &str = include_str!("../../cli/subcommand/bluebook/subcommand.bluebook");

/// Every (name, handler) row the retired subcommand.fixtures carried —
/// canonical order, extracted via `storehouse dump-fixtures` at conversion
/// time. `test` and `enforce-edit` are aliases (same handler, own row).
const EXPECTED: &[(&str, &str)] = &[
    ("storehouse", "run_storehouse"),
    ("lexicon", "run_lexicon"),
    ("terminal", "run_terminal"),
    ("transitional_print", "run_transitional_print"),
    ("heki", "run_heki"),
    ("conceive", "run_conceive"),
    ("develop", "run_develop"),
    ("conceive-behaviors", "run_conceive_behaviors"),
    ("behaviors", "run_behaviors"),
    ("test", "run_behaviors"),
    ("dump-fixtures", "run_dump_fixtures"),
    ("dump-world", "run_dump_world"),
    ("dump-hecksagon", "run_dump_hecksagon"),
    ("specialize", "run_specialize"),
    ("cascade", "run_cascade"),
    ("check-io", "run_check_io"),
    ("check-lifecycle", "run_check_lifecycle"),
    ("check-duplicate-policies", "run_check_duplicate_policies"),
    ("check-all", "run_check_all"),
    ("run", "run_script"),
    ("loop", "run_loop"),
    ("run-loop", "run_pm_loop"),
    ("daemon", "run_daemon"),
    ("macrophage", "run_macrophage"),
    ("enforce-edit", "run_macrophage"),
    ("statusline", "run_statusline"),
    ("is-dispatched", "run_is_dispatched"),
    ("clock", "run_clock"),
    ("sleep", "run_sleep"),
    ("repl", "run_repl"),
];

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
fn establishment_policies_seed_the_full_subcommand_catalog() {
    let boot = parser::parse(BOOT);
    let corpus = parser::parse(SUBCOMMAND);
    let rt = complete_over(boot, corpus, None, vec![], None, "test");

    let recs = rt.all("Subcommand");
    assert_eq!(
        recs.len(),
        EXPECTED.len(),
        "BootCompleted establishment must seed all {} catalog rows (got {})",
        EXPECTED.len(),
        recs.len()
    );
    for (name, handler) in EXPECTED {
        let rec = recs
            .iter()
            .find(|r| bare(r.get("name")) == *name)
            .unwrap_or_else(|| panic!("subcommand `{name}` must establish"));
        assert_eq!(&bare(rec.get("handler")), handler, "handler for `{name}`");
    }

    // Spot-check the value-object source-text round-trip — the argv_shape
    // map with embedded quoted empties is the trickiest with-literal.
    let sh = recs
        .iter()
        .find(|r| bare(r.get("name")) == "storehouse")
        .unwrap();
    assert_eq!(
        bare(sh.get("argv_shape")),
        r#"{ min_positional: 1, max_positional: -1, allowed_kwargs: "", allowed_flags: "" }"#
    );
    assert_eq!(bare(sh.get("dispatch_address")), r#"{ value: "" }"#);
}

#[test]
fn establishment_is_idempotent_on_a_second_boot_completed() {
    let boot = parser::parse(BOOT);
    let corpus = parser::parse(SUBCOMMAND);
    let mut rt = complete_over(boot, corpus, None, vec![], None, "test");

    let _ = rt.dispatch("Boot::BootRun.CompleteBoot", std::collections::HashMap::new());

    assert_eq!(
        rt.all("Subcommand").len(),
        EXPECTED.len(),
        "re-establishment must upsert on :name, never duplicate"
    );
}
