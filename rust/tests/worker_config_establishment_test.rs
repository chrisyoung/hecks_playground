//! worker_config_establishment_test.rs — fixtures→policies site 4 (the
//! daily-musing Cloudflare deploy config) : proves, against the REAL
//! cloudflare.bluebook (include_str! — the shipped deploy declaration, not a
//! test replica), that the `on "BootCompleted"` establishment policy
//! self-seeds the WorkerConfig record whose values previously lived in the
//! `fixture "DailyMusingWorker"` block. The policy must BITE before the
//! fixture dies (Chris, 2026-07-26) : this test is that bite.
//!
//! Also proves the SEAM : the tracked wrangler.toml is byte-identical to
//! what projection::wrangler renders from the ESTABLISHED record — the
//! artifact derives from the record, never from re-parsing the bluebook,
//! and never by hand.
//!
//! [antibody-exempt: rust/tests/worker_config_establishment_test.rs —
//!  kernel-surface integration test asserting Rust runtime state after the
//!  establishment cascade ; same category as agent_defs_establishment_test.rs.]

use storehouse::parser;
use storehouse::run_boot::complete::complete_over;
use storehouse::runtime::Value;

/// The REAL deploy declaration — WorkerConfig + the establishment policy.
const CLOUDFLARE: &str =
    include_str!("../../deployments/daily_musing_cf/cloudflare.bluebook");

/// The tracked artifact the seam must reproduce byte-for-byte.
const WRANGLER: &str =
    include_str!("../../deployments/daily_musing_cf/worker/wrangler.toml");

// A minimal boot domain : the sole thing that matters is `CompleteBoot`
// emits `BootCompleted` (mirrors agent_defs_establishment_test.rs).
const BOOT: &str = r#"Hecks.bluebook "Boot" do
  aggregate "BootRun" do
    attribute :phase, String, default: "pending"
    command "CompleteBoot" do
      role "System"
      emits "BootCompleted"
    end
  end
end"#;

/// Unwrap a runtime Value to its bare string (VO maps unwrap to their
/// `value` member) — same accessor shape as projection::wrangler.
fn bare(v: &Value) -> String {
    match v {
        Value::Str(s) => s.clone(),
        Value::Map(m) => m.get("value").map(bare).unwrap_or_default(),
        other => other.to_string(),
    }
}

#[test]
fn establishment_policy_seeds_the_worker_config() {
    let boot = parser::parse(BOOT);
    let corpus = parser::parse(CLOUDFLARE);
    let rt = complete_over(boot, corpus, None, vec![], None, "test");

    let recs = rt.all("WorkerConfig");
    assert_eq!(
        recs.len(),
        1,
        "BootCompleted establishment must seed exactly one WorkerConfig (got {})",
        recs.len()
    );
    let rec = recs[0];
    let get = |k: &str| bare(rec.get(k));

    // The exact rows the retired fixture carried.
    assert_eq!(get("name"), "daily-musing-worker");
    assert_eq!(get("compatibility_date"), "2026-05-10");
    assert_eq!(get("account_id"), "d64266851dc554da77687e13c058917c");
    assert_eq!(get("main_entrypoint"), "build/worker/shim.mjs");
    assert_eq!(
        get("build_command"),
        "cargo install -q worker-build && worker-build --release"
    );
    assert_eq!(get("r2_binding"), "DAILY_MUSING_R2_BUCKET");
    assert_eq!(get("r2_bucket"), "daily-musing-heki");
    assert_eq!(get("observability"), "enabled");
}

#[test]
fn establishment_is_idempotent_on_a_second_boot_completed() {
    let boot = parser::parse(BOOT);
    let corpus = parser::parse(CLOUDFLARE);
    let mut rt = complete_over(boot, corpus, None, vec![], None, "test");

    let _ = rt.dispatch("Boot::BootRun.CompleteBoot", std::collections::HashMap::new());

    assert_eq!(
        rt.all("WorkerConfig").len(),
        1,
        "re-establishment must upsert on :name, never duplicate"
    );
}

#[test]
fn wrangler_toml_projects_byte_identical_from_the_established_record() {
    let boot = parser::parse(BOOT);
    let corpus = parser::parse(CLOUDFLARE);
    let rt = complete_over(boot, corpus, None, vec![], None, "test");

    let rendered = storehouse::projection::wrangler::render(&rt)
        .expect("an established WorkerConfig must render");
    assert_eq!(
        rendered, WRANGLER,
        "tracked wrangler.toml must equal the seam's render of the established record — \
         re-project with `storehouse project wrangler deployments/daily_musing_cf`"
    );
}
