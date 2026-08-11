//! mindstream_establishment_test.rs — fixtures→policies site 2 (the boot
//! Mindstream + its members) : proves, against the REAL framework
//! mindstream.bluebook (include_str! — the shipped types + establishment
//! policies, not a test replica), that the `on "BootCompleted"` policies
//! self-seed the Mindstream + 14 MindstreamMember records whose rows
//! previously lived in miette/deploy/mindstream.fixtures. The policy must
//! BITE before the fixture dies (Chris, 2026-07-26) : this test is that
//! bite.
//!
//! Also proves the SEAM : projection::procfile renders the Procfile +
//! .overmind.env artifacts from the ESTABLISHED records — byte-identity
//! with the tracked miette/deploy artifacts is asserted when that sibling
//! repo is present (runtime read, parity's optional-miette pattern).
//!
//! [antibody-exempt: rust/tests/mindstream_establishment_test.rs —
//!  kernel-surface integration test asserting Rust runtime state after the
//!  establishment cascade ; same category as agent_defs_establishment_test.rs.]

use storehouse::parser;
use storehouse::run_boot::complete::complete_over;
use storehouse::runtime::Value;

const MINDSTREAM: &str = include_str!(
    "../../hecks_conception/aggregates/framework/mindstream/bluebook/mindstream.bluebook"
);

/// Every member the retired mindstream.fixtures carried, in Procfile
/// order : (name, lifespan kind).
const EXPECTED: &[(&str, &str)] = &[
    ("boot", "one_shot"),
    ("establish", "one_shot"),
    ("heart", "persistent"),
    ("breath", "persistent"),
    ("circadian", "persistent"),
    ("ultradian", "persistent"),
    ("inbox", "persistent"),
    ("inbox_poller", "persistent"),
    ("mind_reconcile", "persistent"),
    ("process_macrophage", "persistent"),
    ("conductor_sweep", "persistent"),
    ("speech_stream", "persistent"),
    ("serve_socket", "persistent"),
    ("consolidate", "persistent"),
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

fn completed() -> storehouse::runtime::Runtime {
    let boot = parser::parse(BOOT);
    let corpus = parser::parse(MINDSTREAM);
    complete_over(boot, corpus, None, vec![], None, "test")
}

#[test]
fn establishment_policies_seed_the_boot_mindstream_and_members() {
    let rt = completed();

    let streams = rt.all("Mindstream");
    assert_eq!(streams.len(), 1, "exactly the boot Mindstream");
    assert_eq!(bare(streams[0].get("name")), "boot");

    let members = rt.all("MindstreamMember");
    assert_eq!(
        members.len(),
        EXPECTED.len(),
        "BootCompleted establishment must seed all {} members (got {})",
        EXPECTED.len(),
        members.len()
    );
    for (i, (name, kind)) in EXPECTED.iter().enumerate() {
        let rec = members
            .iter()
            .find(|r| bare(r.get("name")) == *name)
            .unwrap_or_else(|| panic!("member `{name}` must establish"));
        let lifespan = rec.get("lifespan").to_string();
        assert!(
            lifespan.contains(kind),
            "lifespan kind for `{name}` must be {kind} (got {lifespan})"
        );
        let order: i64 = bare(rec.get("order")).parse().expect("numeric order");
        assert_eq!(order, (i + 1) as i64, "Procfile order for `{name}`");
        assert!(
            !bare(rec.get("command")).is_empty() || rec.get("command").to_string().contains("storehouse"),
            "member `{name}` must carry its command line"
        );
    }
}

#[test]
fn establishment_is_idempotent_on_a_second_boot_completed() {
    let mut rt = completed();
    let _ = rt.dispatch("Boot::BootRun.CompleteBoot", std::collections::HashMap::new());
    assert_eq!(rt.all("MindstreamMember").len(), EXPECTED.len());
    assert_eq!(rt.all("Mindstream").len(), 1);
}

#[test]
fn procfile_and_env_project_byte_identical_from_the_established_records() {
    // The tracked artifacts live in the miette sibling — skip when absent.
    let deploy = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../miette/deploy");
    let (Ok(procfile), Ok(env)) = (
        std::fs::read_to_string(deploy.join("Procfile")),
        std::fs::read_to_string(deploy.join(".overmind.env")),
    ) else {
        eprintln!("skipped — miette sibling repo absent");
        return;
    };
    let rt = completed();
    let arts = storehouse::projection::procfile::render(&rt)
        .expect("established Mindstream + members must render");
    assert_eq!(
        arts.procfile, procfile,
        "tracked Procfile must equal the seam's render — re-project with \
         `storehouse project procfile <conception> --output-dir miette/deploy`"
    );
    assert_eq!(
        arts.env, env,
        "tracked .overmind.env must equal the seam's render — re-project with \
         `storehouse project procfile <conception> --output-dir miette/deploy`"
    );
}
