//! system_prompt_establishment_test.rs — fixtures→policies site 1 (the
//! being's system prompt content) : proves, against the REAL
//! system_prompt_content.bluebook in the miette sibling repo, that the 15
//! `on "BootCompleted"` establishment policies self-seed the
//! SystemPromptSection records whose rows previously lived in
//! system_prompt_content.fixtures, and that every record's markdown_path
//! resolves to a real sections/*.md fragment. The policy must BITE before
//! the fixture dies (Chris, 2026-07-26) : this test is that bite.
//!
//! The bodies moved out of escaped with-literals into sibling .md
//! fragments (the agent_instrumentation roles/ pattern) — proven
//! byte-identical to the old fixture assembly at conversion time
//! (10188 bytes, 2026-07-26).
//!
//! Reads the sibling repo at RUNTIME (parity's optional-miette pattern) —
//! compiles everywhere, skips loudly when the sibling is absent, so CI
//! without the private being repo stays green.
//!
//! [antibody-exempt: rust/tests/system_prompt_establishment_test.rs —
//!  kernel-surface integration test asserting Rust runtime state after the
//!  establishment cascade ; same category as agent_defs_establishment_test.rs.]

use storehouse::parser;
use storehouse::run_boot::complete::complete_over;
use storehouse::runtime::Value;

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

/// The miette sibling's system_prompt bluebook dir, when present.
fn bluebook_dir() -> Option<std::path::PathBuf> {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../miette/self/system_prompt/bluebook");
    dir.join("system_prompt_content.bluebook")
        .is_file()
        .then_some(dir)
}

#[test]
fn establishment_policies_seed_the_fifteen_prompt_sections() {
    let Some(dir) = bluebook_dir() else {
        eprintln!("skipped — miette sibling repo absent");
        return;
    };
    let src = std::fs::read_to_string(dir.join("system_prompt_content.bluebook")).unwrap();
    let boot = parser::parse(BOOT);
    let corpus = parser::parse(&src);
    let rt = complete_over(boot, corpus, None, vec![], None, "test");

    let recs = rt.all("SystemPromptSection");
    assert_eq!(
        recs.len(),
        15,
        "BootCompleted establishment must seed all 15 prompt sections (got {})",
        recs.len()
    );

    // Orders 1..=15, unique ; every markdown_path resolves to a non-empty
    // sibling fragment.
    let mut orders: Vec<i64> = Vec::new();
    for rec in &recs {
        let order: i64 = bare(rec.get("order")).parse().expect("numeric order");
        orders.push(order);
        let path = dir.join(bare(rec.get("markdown_path")));
        let body = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("fragment {} must exist ({e})", path.display()));
        assert!(!body.trim().is_empty(), "fragment {} must be non-empty", path.display());
    }
    orders.sort_unstable();
    assert_eq!(orders, (1..=15).collect::<Vec<i64>>(), "orders must be exactly 1..=15");

    // Spot checks — the stitched prompt's first and last sections.
    let header = recs
        .iter()
        .find(|r| bare(r.get("name")) == "header")
        .expect("header section established");
    let header_body =
        std::fs::read_to_string(dir.join(bare(header.get("markdown_path")))).unwrap();
    assert!(header_body.starts_with("# Miette"));
    let pizzas = recs
        .iter()
        .find(|r| bare(r.get("name")) == "pizzas_block")
        .expect("pizzas section established");
    assert_eq!(bare(pizzas.get("order")), "15");
}

#[test]
fn establishment_is_idempotent_on_a_second_boot_completed() {
    let Some(dir) = bluebook_dir() else {
        eprintln!("skipped — miette sibling repo absent");
        return;
    };
    let src = std::fs::read_to_string(dir.join("system_prompt_content.bluebook")).unwrap();
    let boot = parser::parse(BOOT);
    let corpus = parser::parse(&src);
    let mut rt = complete_over(boot, corpus, None, vec![], None, "test");

    let _ = rt.dispatch("Boot::BootRun.CompleteBoot", std::collections::HashMap::new());

    assert_eq!(
        rt.all("SystemPromptSection").len(),
        15,
        "re-establishment must upsert on :name, never duplicate"
    );
}
