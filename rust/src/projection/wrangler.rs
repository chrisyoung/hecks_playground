//! projection::wrangler — emit a Cloudflare `wrangler.toml` from the
//! ESTABLISHED WorkerConfig record.
//!
//! The seam (fixtures→policies, 2026-07-26) : cloudflare.bluebook DECLARES
//! the worker config, its `on "BootCompleted"` establishment policy ASSERTS
//! the WorkerConfig record (idempotent upsert on :name), and this projector
//! RENDERS the artifact from the record — never by re-parsing the bluebook.
//! Replaces the retired `fixture "DailyMusingWorker"` block, whose values
//! reached wrangler.toml by hand.
//!
//! Pure — no file I/O ; the caller (`storehouse project wrangler <root>`)
//! owns loading the corpus, routing BootCompleted, and writing the emitted
//! string. Byte-identity with the tracked
//! deployments/daily_musing_cf/worker/wrangler.toml is enforced by
//! rust/tests/worker_config_establishment_test.rs.

use crate::runtime::{Runtime, Value};

/// Unwrap a runtime Value to its bare string (VO maps unwrap to their
/// `value` member) — same accessor shape as run_boot/agent_defs.rs.
fn bare(v: &Value) -> String {
    match v {
        Value::Str(s) => s.clone(),
        Value::Map(m) => m.get("value").map(bare).unwrap_or_default(),
        other => other.to_string(),
    }
}

/// Render wrangler.toml for the established WorkerConfig record.
/// None when no record is established — the caller reports loudly
/// (an unestablished config must never silently emit an empty artifact).
pub fn render(rt: &Runtime) -> Option<String> {
    let recs = rt.all("WorkerConfig");
    let rec = recs.first()?;
    let get = |k: &str| bare(rec.get(k));

    let mut out = String::new();
    out.push_str("# DERIVED from ../cloudflare.bluebook — the ESTABLISHED WorkerConfig\n");
    out.push_str("# record — by `storehouse project wrangler`. The bluebook declares, the\n");
    out.push_str("# BootCompleted establishment policy asserts the record, this artifact\n");
    out.push_str("# projects from the record. Do not hand-edit — change the policy and\n");
    out.push_str("# re-project.\n\n");
    out.push_str(&format!("name               = \"{}\"\n", get("name")));
    out.push_str(&format!("main               = \"{}\"\n", get("main_entrypoint")));
    out.push_str(&format!("compatibility_date = \"{}\"\n", get("compatibility_date")));

    let binding = get("r2_binding");
    let bucket = get("r2_bucket");
    if !binding.is_empty() && !bucket.is_empty() {
        out.push_str("\n# heki -> R2 persistence binding. lib.rs reads this binding by\n");
        out.push_str("# name to hydrate state on boot and persist every dispatch back.\n");
        out.push_str("[[r2_buckets]]\n");
        out.push_str(&format!("binding     = \"{}\"\n", binding));
        out.push_str(&format!("bucket_name = \"{}\"\n", bucket));
    }

    let build = get("build_command");
    if !build.is_empty() {
        out.push_str("\n[build]\n");
        out.push_str(&format!("command = \"{}\"\n", build));
    }

    out.push_str("\n[observability]\n");
    out.push_str(&format!(
        "enabled = {}\n",
        if get("observability") == "enabled" { "true" } else { "false" }
    ));
    Some(out)
}
