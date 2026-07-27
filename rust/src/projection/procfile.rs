//! projection::procfile — emit the overmind Procfile + .overmind.env from
//! the ESTABLISHED Mindstream + MindstreamMember records.
//!
//! The seam (fixtures→policies, 2026-07-26) : the framework
//! mindstream.bluebook DECLARES the boot mindstream + members as
//! `on "BootCompleted"` establishment policies (Define / Register upsert
//! on :name), and this projector RENDERS both artifacts from the records —
//! never by parsing a fixtures file. Replaces the never-shipped
//! `storehouse specialize procfile --fixtures` referenced by the old
//! artifact headers, and retires miette/deploy/mindstream.fixtures.
//!
//! Pure — no file I/O ; the caller (`storehouse project procfile <root>
//! [--output-dir <dir>]`) owns loading the corpus, routing BootCompleted,
//! and writing the emitted strings. Byte-identity with the tracked
//! miette/deploy artifacts is enforced by
//! rust/tests/mindstream_establishment_test.rs.

use crate::runtime::{Runtime, Value};

pub struct ProcfileArtifacts {
    pub procfile: String,
    pub env: String,
}

/// Unwrap a runtime Value to its bare string (VO maps unwrap to their
/// `value` member) — same accessor shape as run_boot/agent_defs.rs.
fn bare(v: &Value) -> String {
    match v {
        Value::Str(s) => s.clone(),
        Value::Map(m) => m.get("value").map(bare).unwrap_or_default(),
        other => other.to_string(),
    }
}

/// Extract `key: "…"` from a value-object SOURCE-TEXT token (with-literals
/// carry `{ line: "…" }` / `{ kind: "one_shot", … }` verbatim — the same
/// shape the retired fixtures parser captured).
fn quoted_field(src: &str, key: &str) -> Option<String> {
    let marker = format!("{key}: \"");
    let start = src.find(&marker)? + marker.len();
    let rest = &src[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// Render both overmind artifacts from the established records. None when
/// no Mindstream or no members are established — the caller reports loudly
/// (an unestablished stream must never silently emit empty artifacts).
pub fn render(rt: &Runtime) -> Option<ProcfileArtifacts> {
    let streams = rt.all("Mindstream");
    let stream = streams.first()?;
    let members = rt.all("MindstreamMember");
    if members.is_empty() {
        return None;
    }

    // Deterministic Procfile order — numeric :order carried by every
    // Register policy, never store iteration order.
    let mut rows: Vec<(i64, String, String, String)> = members
        .iter()
        .map(|rec| {
            let order = bare(rec.get("order")).parse::<i64>().unwrap_or(i64::MAX);
            let name = bare(rec.get("name"));
            let command_src = bare(rec.get("command"));
            let line = quoted_field(&command_src, "line").unwrap_or(command_src.clone());
            let kind = quoted_field(&bare(rec.get("lifespan")), "kind").unwrap_or_default();
            (order, name, line, kind)
        })
        .collect();
    rows.sort_by_key(|(order, _, _, _)| *order);

    let mut procfile = String::new();
    procfile.push_str("# Generated artifact — do not hand-edit. Source of truth :\n");
    procfile.push_str("#   hecks_conception/aggregates/framework/mindstream/bluebook/mindstream.bluebook\n");
    procfile.push_str("#   (the Mindstream + MindstreamMember types AND the BootCompleted establishment\n");
    procfile.push_str("#   policies that seed the boot rows — fixtures→policies, 2026-07-26).\n");
    procfile.push_str("# Regenerate : storehouse project procfile <conception-root> --output-dir <deploy-dir>\n");
    procfile.push_str("# One line per ESTABLISHED MindstreamMember (name: command, verbatim, in :order) ;\n");
    procfile.push_str("# one_shot members flow into OVERMIND_CAN_DIE in the sibling .overmind.env.\n");
    for (_, name, line, _) in &rows {
        procfile.push_str(&format!("{name}: {line}\n"));
    }

    let can_die: Vec<&str> = rows
        .iter()
        .filter(|(_, _, _, kind)| kind == "one_shot")
        .map(|(_, name, _, _)| name.as_str())
        .collect();

    let mut env = String::new();
    env.push_str("# Generated artifact — do not hand-edit. Source of truth :\n");
    env.push_str("#   hecks_conception/aggregates/framework/mindstream/bluebook/mindstream.bluebook\n");
    env.push_str("#   (the BootCompleted establishment policies — fixtures→policies, 2026-07-26).\n");
    env.push_str("# Regenerate : storehouse project procfile <conception-root> --output-dir <deploy-dir>\n");
    env.push_str("# OVERMIND_CAN_DIE lists every MindstreamMember whose lifespan.kind == \"one_shot\" ;\n");
    env.push_str("# the framework env (HECKS_DAEMON, HECKS_EVENT_SOURCING) is what the boot pipeline needs ;\n");
    env.push_str("# HECKS_CONCEPTION_DIR locates the conception (the engine now lives outside the tree) ;\n");
    env.push_str("# HECKS_ADDITIONAL_CORPUS_ROOTS carries the being-repo bluebook roots whose\n");
    env.push_str("# BootCompleted policies must register in the completion corpus.\n");
    env.push('\n');
    env.push_str(&format!("OVERMIND_CAN_DIE={}\n", can_die.join(",")));
    env.push_str("HECKS_DAEMON=1\n");
    env.push_str("HECKS_EVENT_SOURCING=1\n");
    let conception = quoted_field(&bare(stream.get("directory")), "path").unwrap_or_default();
    env.push_str(&format!("HECKS_CONCEPTION_DIR={conception}\n"));
    let roots = quoted_field(&bare(stream.get("corpus_roots")), "value")
        .unwrap_or_else(|| bare(stream.get("corpus_roots")));
    if !roots.is_empty() && !roots.starts_with('{') {
        env.push_str(&format!("HECKS_ADDITIONAL_CORPUS_ROOTS={roots}\n"));
    }
    Some(ProcfileArtifacts { procfile, env })
}
