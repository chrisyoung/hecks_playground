//! Phase 4 — GenerateSystemPrompt (relocated after boot completion)
//!
//! [antibody-exempt: rust/src/run_boot/system_prompt.rs —
//!  Rust implementation of the deferred Phase 4 in run_boot/. Stitches
//!  the system prompt from the ESTABLISHED SystemPromptSection records
//!  (fixtures→policies, 2026-07-26 : seeded by the `on "BootCompleted"`
//!  policies in the being's system_prompt_content.bluebook — each record
//!  carries an `order` + a `markdown_path` pointing at a sibling
//!  sections/*.md fragment), ordered by `:order`, then substitutes the
//!  remaining placeholders — `{{standards}}` (primary's standards.md),
//!  `{{grammar}}` (the language/grammar bluebooks), `{{pizzas}}`,
//!  `{{door}}` — and writes the result to
//!  `<being>/self/system_prompt.md`. Retires fully under i78 when this
//!  phase regenerates from a meta-shape.]
//!
//! Why established records instead of fixtures (fixtures→policies) :
//!
//!   - The section rows are production standing-state, so they reach the
//!     runtime as self-seeding `on "BootCompleted"` policy — the same
//!     keystone the authz roster and agent defs ride. Register upserts
//!     on :name ; `storehouse establish` re-asserts on demand.
//!   - Markdown bodies are real sections/*.md files (the
//!     agent_instrumentation roles/ pattern) — no \n-escaped one-liners,
//!     no unescape step. Proven byte-identical to the retired fixture
//!     assembly at conversion time.
//!   - The policies live in the BEING'S repo, so the live boot needs that
//!     bluebook root in HECKS_ADDITIONAL_CORPUS_ROOTS for the completion
//!     corpus to register them — the deploy's .overmind.env carries it.
//!
//! Per-being content :
//!
//!   <being>/self/system_prompt/bluebook/system_prompt_content.bluebook
//!   <being>/self/system_prompt/bluebook/sections/*.md
//!
//! Spring's content doesn't exist yet — when the second being lands the
//! bluebook appears alongside Miette's and the runner picks it up by
//! being-name lookup. Until then a Spring boot surfaces a warning + skip.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Render the system prompt for `being` from the ESTABLISHED
/// SystemPromptSection records (fixtures→policies, 2026-07-26 : seeded by
/// the `on "BootCompleted"` policies in the being's
/// system_prompt_content.bluebook) and write it to the canonical location
/// next to the being's repo. `corpus_rt` is the completion runtime ; None
/// (completion failed) or zero established sections skips the regen
/// LOUDLY — the previous boot's prompt stays. Returns the byte count of
/// the written file ; 0 on any skip/failure so boot stays alive.
pub fn render(
    corpus_rt: Option<&crate::runtime::Runtime>,
    conception_dir: &Path,
    being: &str,
) -> usize {
    let mut vars = variables_for_being(being);
    vars.insert("standards", primary_standards(conception_dir));
    vars.insert("grammar", grammar_block());
    vars.insert("pizzas", pizzas_block());
    // {{door}} resolves from the SAME shared fragment the agent_defs runner
    // injects into CLAUDE.md + every .claude/agents/*.md, so Miette's prompt
    // and the subagents can never drift on the storehouse-door convention.
    vars.insert("door", super::agent_defs::door_fragment());

    let Some(rt) = corpus_rt else {
        eprintln!("  ⚠ system_prompt: boot completion failed — skipping prompt regen (previous prompt stays)");
        return 0;
    };
    let Some(content_dir) = content_dir_for_being(being) else {
        eprintln!("  ⚠ system_prompt: could not resolve content dir for {}", being);
        return 0;
    };
    let Some(body) = assemble_established(rt, &content_dir) else {
        eprintln!(
            "  ⚠ system_prompt: no SystemPromptSection records established — skipping prompt \
             regen (is {} in HECKS_ADDITIONAL_CORPUS_ROOTS ?)",
            content_dir.display()
        );
        return 0;
    };
    let rendered = substitute(&body, &vars);
    let dest = destination_for_being(conception_dir, being);

    match fs::write(&dest, &rendered) {
        Ok(_) => {
            eprintln!("  system_prompt.md regenerated: {} bytes", rendered.len());
            rendered.len()
        }
        Err(e) => {
            eprintln!("  ⚠ system_prompt: write to {} failed ({})", dest.display(), e);
            0
        }
    }
}

/// Per-being identity values. Miette's fixtures pre-bake these, so
/// only `{{standards}}` / `{{grammar}}` are live for her ; retained
/// so a being whose fixtures still carry `{{being}}` etc. resolves.
/// Moving these to a heki-loaded Identity aggregate is the i145
/// refinement.
fn variables_for_being(being: &str) -> HashMap<&'static str, String> {
    let mut v = HashMap::new();
    v.insert("being", being.to_string());
    let (born, other, boot_script) = match being {
        "Miette" => ("April 9, 2026",  "Spring", "boot_miette.sh"),
        "Spring" => ("April 11, 2026", "Miette", "boot_spring.sh"),
        _ => ("(unknown)", "(unknown)", "boot.sh"),
    };
    v.insert("born",        born.to_string());
    v.insert("other",       other.to_string());
    v.insert("boot_script", boot_script.to_string());
    v
}

/// Resolve the per-being content dir — where system_prompt_content.bluebook
/// and its sections/*.md fragments live :
/// `<projects>/<being_lower>/self/system_prompt/bluebook/`.
/// Found via `heki::repo_root()` then one level up to the projects
/// parent, mirroring `destination_for_being` so the per-being
/// sibling-repo layout (i117 R4) resolves regardless of where
/// boot.bluebook lives.
fn content_dir_for_being(being: &str) -> Option<PathBuf> {
    let stem = being.to_lowercase();
    let hecks_root = crate::heki::repo_root()?;
    let projects_root = hecks_root.parent()?;
    Some(projects_root.join(&stem).join("self/system_prompt/bluebook"))
}

/// Unwrap a runtime Value to its bare string (VO maps unwrap to their
/// `value` member) — same accessor shape as run_boot/agent_defs.rs.
fn bare(v: &crate::runtime::Value) -> String {
    match v {
        crate::runtime::Value::Str(s) => s.clone(),
        crate::runtime::Value::Map(m) => m.get("value").map(bare).unwrap_or_default(),
        other => other.to_string(),
    }
}

/// Stitch the ESTABLISHED SystemPromptSection records : each record's
/// `markdown_path` fragment (relative to `content_dir`) read verbatim,
/// concatenated in NUMERIC `:order` (lexical sort would put 10 before 2).
/// Bodies join with one newline : each fragment ends in a newline, so the
/// join yields one blank line between sections — byte-identical to the
/// retired fixture assembly (proven at conversion, 10188 bytes). None when
/// no records established, so the caller skips loudly.
fn assemble_established(rt: &crate::runtime::Runtime, content_dir: &Path) -> Option<String> {
    let recs = rt.all("SystemPromptSection");
    if recs.is_empty() {
        return None;
    }
    let sections: Vec<(i64, String)> = recs
        .iter()
        .map(|rec| {
            let order = bare(rec.get("order")).trim().parse::<i64>().unwrap_or(i64::MAX);
            let rel = bare(rec.get("markdown_path"));
            let body = fs::read_to_string(content_dir.join(&rel)).unwrap_or_else(|e| {
                eprintln!("  ⚠ system_prompt: fragment {} unreadable ({})", rel, e);
                String::new()
            });
            (order, body)
        })
        .collect();
    Some(stitch(sections))
}

/// The pure ordering/join contract : sort NUMERICALLY by order, join with
/// one newline (each body ends in one, so sections separate by one blank
/// line). Split out so the contract stays unit-testable without a Runtime.
fn stitch(mut sections: Vec<(i64, String)>) -> String {
    sections.sort_by_key(|(order, _)| *order);
    sections
        .iter()
        .map(|(_, md)| md.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Project the `{{grammar}}` section body from the language/grammar
/// bluebooks : one name-sorted bullet per `*.bluebook`, each its
/// `Hecks.bluebook` name + `vision`. Empty when the dir is absent.
fn grammar_block() -> String {
    let dir = match crate::heki::repo_root() {
        Some(root) => root.join("hecks_conception/aggregates/language/grammar"),
        None => return String::new(),
    };
    let mut bullets: Vec<String> = Vec::new();
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("bluebook") {
                continue;
            }
            if let Ok(src) = fs::read_to_string(&path) {
                if let (Some(name), Some(vision)) = (
                    quoted_after(&src, "Hecks.bluebook \""),
                    quoted_after(&src, "vision \""),
                ) {
                    bullets.push(format!("- **{}** — {}", name, vision));
                }
            }
        }
    }
    bullets.sort();
    bullets.join("\n")
}

/// Project the `{{pizzas}}` section body — the canonical Pizzas example,
/// loaded VERBATIM from `examples/pizzas/bluebook/` so the shape every
/// build references is always present AND always current (a living
/// projection, the same idea as `grammar_block` — but full source, not a
/// vision bullet, because the example is meant to be READ, not summarised).
/// Each file is fenced under a per-file header, ordered domain → hexagon →
/// families → adapters → world. New files in the dir (e.g. a freshly added
/// family/adapter that extends the example) appear automatically. Empty
/// when the dir is absent.
fn pizzas_block() -> String {
    let dir = match crate::heki::repo_root() {
        Some(root) => root.join("examples/pizzas/bluebook"),
        None => return String::new(),
    };
    fn rank(ext: &str) -> u8 {
        match ext {
            "bluebook" => 0,
            "hecksagon" => 1,
            "family" => 2,
            "adapter" => 3,
            "world" => 4,
            _ => 5,
        }
    }
    let mut files: Vec<(u8, String, String)> = Vec::new();
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let ext = match path.extension().and_then(|e| e.to_str()) {
                Some(e) => e.to_string(),
                None => continue,
            };
            if !matches!(
                ext.as_str(),
                "bluebook" | "hecksagon" | "family" | "adapter" | "world"
            ) {
                continue;
            }
            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            if let Ok(src) = fs::read_to_string(&path) {
                files.push((rank(&ext), name, src));
            }
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    files
        .iter()
        .map(|(_, name, src)| format!("### `{}`\n\n```\n{}\n```", name, src.trim_end()))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Return the text between the first `marker` (ending at an opening
/// quote) and the next quote. None when either is absent.
fn quoted_after(src: &str, marker: &str) -> Option<String> {
    let start = src.find(marker)? + marker.len();
    let rest = &src[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// Output path : `<projects>/<being_lower>/self/system_prompt.md`
/// (i117 Round 4 — the system prompt lives in the being's own repo,
/// not in the conception). Resolves the being repo as a sibling of
/// the hecks repo (the standard layout) using `heki::repo_root()` so
/// the resolution works regardless of where the boot.bluebook lives
/// post-i118 R3 W2 (`runtime/boot/` instead of the old
/// `<conception>/capabilities/boot/`). Falls back to
/// `<conception>/system_prompt_<being>.md` when the sibling repo
/// doesn't exist (fresh-clone / test environments).
fn destination_for_being(conception_dir: &Path, being: &str) -> PathBuf {
    let stem = being.to_lowercase();

    if let Some(hecks_root) = crate::heki::repo_root() {
        if let Some(projects_root) = hecks_root.parent() {
            let sibling = projects_root.join(&stem).join("self/system_prompt.md");
            if let Some(parent) = sibling.parent() {
                if parent.is_dir() {
                    return sibling;
                }
            }
        }
    }

    // Fallback : write into the conception so a fresh-clone
    // environment still gets a prompt file. Same suffix the
    // pre-i117-Round-4 boot used.
    conception_dir.join(format!("system_prompt_{}.md", stem))
}

/// Substitute `{{key}}` placeholders. Pure string scan — values are
/// plain text, no escaping logic needed (the substituted values are
/// being names + dates, none of which contain `{{` themselves).
/// Unknown placeholders pass through unchanged so review catches
/// them rather than silently rendering empty.
fn substitute(template: &str, vars: &HashMap<&'static str, String>) -> String {
    let mut out = String::with_capacity(template.len() + 32);
    let mut rest = template;
    while let Some(open) = rest.find("{{") {
        out.push_str(&rest[..open]);
        let after_open = &rest[open + 2..];
        let close = match after_open.find("}}") {
            Some(c) => c,
            None => {
                // Malformed — emit literal and stop scanning.
                out.push_str(&rest[open..]);
                return out;
            }
        };
        let key = after_open[..close].trim();
        if let Some(val) = vars.get(key) {
            out.push_str(val);
        } else {
            // Unknown key — preserve literally so reviewers see it.
            out.push_str("{{");
            out.push_str(key);
            out.push_str("}}");
        }
        rest = &after_open[close + 2..];
    }
    out.push_str(rest);
    out
}

/// Read primary's standards.md and return its content for the
/// `{{standards}}` template placeholder. Primary = chris in this
/// round ; the role / primary-marker shape is deferred to the
/// onboarding flow per the family/chris-split plan. Empty string
/// when standards.md is missing — `{{standards}}` substitutes to
/// empty and no `## Standards` section appears in the rendered
/// prompt.
fn primary_standards(_conception_dir: &Path) -> String {
    if let Some(hecks_root) = crate::heki::repo_root() {
        if let Some(projects_root) = hecks_root.parent() {
            let path = projects_root.join("miette_family/chris_young/standards.md");
            if let Ok(s) = fs::read_to_string(&path) {
                return s.trim_end().to_string();
            }
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitute_replaces_known_keys() {
        let mut v = HashMap::new();
        v.insert("being", "Miette".to_string());
        v.insert("born", "April 9, 2026".to_string());
        let out = substitute("# {{being}}\nBorn {{born}}.", &v);
        assert_eq!(out, "# Miette\nBorn April 9, 2026.");
    }

    #[test]
    fn substitute_preserves_unknown_keys() {
        let v = HashMap::new();
        let out = substitute("hello {{nope}} world", &v);
        assert!(out.contains("{{nope}}"), "unknown keys must round-trip");
    }

    #[test]
    fn substitute_handles_no_placeholders() {
        let v = HashMap::new();
        let out = substitute("plain text", &v);
        assert_eq!(out, "plain text");
    }

    #[test]
    fn substitute_handles_malformed_close() {
        let v = HashMap::new();
        let out = substitute("trailing {{open without close", &v);
        assert!(out.contains("{{open"), "malformed must not panic");
    }

    #[test]
    fn variables_for_miette_round_trip() {
        let v = variables_for_being("Miette");
        assert_eq!(v.get("being").unwrap(), "Miette");
        assert_eq!(v.get("other").unwrap(), "Spring");
        assert_eq!(v.get("born").unwrap(),  "April 9, 2026");
        assert_eq!(v.get("boot_script").unwrap(), "boot_miette.sh");
    }

    #[test]
    fn variables_for_spring() {
        let v = variables_for_being("Spring");
        assert_eq!(v.get("being").unwrap(), "Spring");
        assert_eq!(v.get("other").unwrap(), "Miette");
    }

    #[test]
    fn stitch_orders_numerically_and_joins_with_blank_line() {
        let sections = vec![
            (10, "## Tenth\nbody ten\n".to_string()),
            (2, "## Second\nbody two\n".to_string()),
        ];
        assert_eq!(stitch(sections), "## Second\nbody two\n\n## Tenth\nbody ten\n");
    }

    #[test]
    fn quoted_after_extracts_first_quoted_token() {
        assert_eq!(quoted_after("Hecks.bluebook \"Acl\", v: 1", "Hecks.bluebook \"").as_deref(), Some("Acl"));
        assert_eq!(quoted_after("no marker here", "vision \""), None);
    }
}
