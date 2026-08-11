//! Regression pin : a single-FILE dispatch must DELIVER its argv payload.
//!
//! `storehouse <file.bluebook> <Domain::Aggregate.Command> k=v …` parsed its
//! `k=v` pairs into `attrs` and then called `rt.dispatch(cmd, HashMap::new())`,
//! throwing every one away. The command still ran, still minted a record, and
//! still printed `ok:true` — with every field absent.
//!
//! It stayed invisible because absence is LEGAL at every gate it passes :
//! the payload gate's invariant arm skips a missing attr on purpose (that is
//! the required arm's concern), and an unresolved bare name in a `given` falls
//! through to `Value::Str("word")` rather than refusing. So the two things that
//! should have caught it both read the empty payload as a well-formed one. It
//! cost two separate investigations before this pin existed : the `%w[]`
//! literal-set work (whose live repro could not be made to refuse, though the
//! interpreter judged it correctly in isolation) and finding 1 of
//! `inbox/given-gate-fails-open-card.md` ("command attributes do not reach a
//! given"). Both were the transport, not the judge.
//!
//! The sibling DIRECTORY branch (`dispatch_hecksagon`) always passed attrs
//! through, which is why the MCP door and `storehouse <root> …` were unaffected
//! — and why nothing in the suite noticed. The broken path was the one only a
//! human types, including the example in README.md.
//!
//! Every in-process gate test boots `Runtime` directly and so enters BELOW this
//! transport ; they cannot arbitrate it. This spawns the REAL binary so the
//! whole path (argv → k=v parse → Value::Str → payload gate / givens) is pinned.
//!
//! Usage : `cargo test --release --test single_file_payload_argv_test`

use std::path::PathBuf;
use std::process::Command;

fn binary() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("target");
    p.push(if cfg!(debug_assertions) { "debug" } else { "release" });
    p.push("storehouse");
    p
}

fn tmpdir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "sf_payload_{}_{}_{}",
        tag,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

const PROBE: &str = r##"Hecks.bluebook "SfProbe" do
  aggregate "Probe" do
    description "One VO invariant and one given, both reading the incoming payload"
    attribute :mode, Mode
    attribute :word, String

    value_object "Mode" do
      attribute :value, String
      invariant "mode must be one of the known modes" do
        %w[ensure status stop].include?(value)
      end
    end

    command "SetMode" do
      role "Operator"
      attribute :mode, Mode
    end

    command "WordEquals" do
      role "Operator"
      attribute :word, String
      given("word is Hi") { word == "Hi" }
    end
  end
end
"##;

/// Writes the probe into its own dir so each case gets a fresh data dir.
fn probe_file(tag: &str) -> PathBuf {
    let dir = tmpdir(tag);
    let file = dir.join("sf_probe.bluebook");
    std::fs::write(&file, PROBE).unwrap();
    file
}

fn dispatch(file: &PathBuf, cmd: &str, pair: &str) -> (bool, String) {
    let out = Command::new(binary())
        .arg(file)
        .arg(cmd)
        .arg(pair)
        .output()
        .expect("storehouse binary runs");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.success(), combined)
}

#[test]
fn vo_invariant_judges_the_argv_payload() {
    // The value must ARRIVE for the invariant to judge it. Before the fix the
    // payload was empty, the gate took its "absence passes" arm, and `banana`
    // was accepted — looking exactly like an invariant that held.
    let file = probe_file("inv");
    let (ok, out) = dispatch(&file, "SfProbe::Probe.SetMode", "mode=banana");
    assert!(!ok, "mode=banana must be REFUSED, got success. output:\n{}", out);
    assert!(
        out.contains("PayloadInvariantViolation"),
        "refusal must name the invariant. output:\n{}",
        out
    );
}

#[test]
fn vo_invariant_admits_an_in_set_argv_payload() {
    // The other half of the pin. A gate that refuses everything would satisfy
    // the test above and be just as broken — a rule must be able to PASS.
    let file = probe_file("inv_ok");
    let (ok, out) = dispatch(&file, "SfProbe::Probe.SetMode", "mode=stop");
    assert!(ok, "mode=stop must be ACCEPTED. output:\n{}", out);
}

#[test]
fn given_reads_the_argv_payload() {
    // Finding 1 of inbox/given-gate-fails-open-card.md. With the payload
    // dropped, `word` resolved to the literal string "word" — never equal to
    // "Hi" — so the given failed for every input, including the valid one.
    let file = probe_file("given");
    let (ok, out) = dispatch(&file, "SfProbe::Probe.WordEquals", "word=Hi");
    assert!(ok, "word=Hi must satisfy the given. output:\n{}", out);
}

#[test]
fn given_still_refuses_a_wrong_argv_payload() {
    // And it must still DISCRIMINATE : delivering the payload has to make the
    // given answer the question, not merely stop failing.
    let file = probe_file("given_no");
    let (ok, out) = dispatch(&file, "SfProbe::Probe.WordEquals", "word=Bye");
    assert!(!ok, "word=Bye must be REFUSED, got success. output:\n{}", out);
    assert!(
        out.contains("GivenFailed"),
        "refusal must name the given. output:\n{}",
        out
    );
}
