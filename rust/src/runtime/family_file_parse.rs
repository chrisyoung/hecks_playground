//! family_file_parse — the dedicated readers for framework hecksagons
//! (i557) : parse_adapter_family_file / parse_behavior_kind_file pull the
//! inner DSL (fields, trigger_field, response_field, behavior, providers ;
//! required_fields, trigger_attribute) that the main Hecksagon parser
//! skips, plus the string leaves (extract_between_quotes, strip_keyword,
//! first_symbol, collect_symbols). Not a full Ruby parser — just enough
//! for the shapes these hecksagons actually use.
//!
//! Cask extracted VERBATIM from runtime/framework_registry.rs
//! (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/family_file_parse.rs — kernel-floor
//!  registry readers, relocated verbatim from framework_registry.rs
//!  blanket.]

use super::framework_registry::{AdapterFamily, BehaviorKind};

// ── Dedicated readers for framework hecksagons ──────────────────────
//
// Per i557 the main Hecksagon parser captures `framework_kind` on the
// IR but skips the inner DSL of adapter_family / behavior_kind blocks.
// These helpers read what we need (the inner declarations) without
// touching the main parser. They handle the small shape these
// hecksagons actually use ; not a full Ruby parser, just enough to
// pull fields / trigger_field / response_field / behavior / providers
// for `Hecks.adapter_family`, and required_fields / trigger_attribute
// for `Hecks.behavior_kind`. Comments and blank lines are skipped.

pub(super) fn parse_adapter_family_file(source: &str) -> Option<AdapterFamily> {
    let mut fam = AdapterFamily::default();
    let mut found_header = false;

    for raw_line in source.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line.starts_with("Hecks.adapter_family") {
            if let Some(n) = extract_between_quotes(line) {
                fam.name = n;
                found_header = true;
            }
            continue;
        }

        // fields :a, :b, :c, ...
        if line.starts_with("fields ") || line.starts_with("fields\t") {
            let rest = &line["fields".len()..];
            collect_symbols(rest, &mut fam.fields);
            continue;
        }

        // trigger_field :command
        if let Some(rest) = strip_keyword(line, "trigger_field") {
            if let Some(s) = first_symbol(rest) {
                fam.trigger_field = Some(s);
            }
            continue;
        }

        // response_field :result_into
        if let Some(rest) = strip_keyword(line, "response_field") {
            if let Some(s) = first_symbol(rest) {
                fam.response_field = Some(s);
            }
            continue;
        }

        // behavior :invoke_claude_tool  (singular ; per claude_tool family)
        if let Some(rest) = strip_keyword(line, "behavior") {
            if let Some(s) = first_symbol(rest) {
                if fam.behavior.is_none() {
                    fam.behavior = Some(s.clone());
                }
                if !fam.behaviors.contains(&s) {
                    fam.behaviors.push(s);
                }
            }
            continue;
        }

        // behaviors :a, :b   (plural ; multi-behavior families)
        if let Some(rest) = strip_keyword(line, "behaviors") {
            let mut all = Vec::new();
            collect_symbols(rest, &mut all);
            for b in &all {
                if !fam.behaviors.contains(b) {
                    fam.behaviors.push(b.clone());
                }
            }
            if fam.behavior.is_none() {
                if let Some(first) = all.first() {
                    fam.behavior = Some(first.clone());
                }
            }
            continue;
        }

        // providers :twilio, :vonage
        if let Some(rest) = strip_keyword(line, "providers") {
            collect_symbols(rest, &mut fam.providers);
            continue;
        }
    }

    if found_header && !fam.name.is_empty() {
        Some(fam)
    } else {
        None
    }
}

pub(super) fn parse_behavior_kind_file(source: &str) -> Option<BehaviorKind> {
    let mut bk = BehaviorKind::default();
    let mut found_header = false;

    for raw_line in source.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line.starts_with("Hecks.behavior_kind") {
            if let Some(n) = extract_between_quotes(line) {
                bk.name = n;
                found_header = true;
            }
            continue;
        }

        // requires_field :tool   (one per line ; can repeat)
        if let Some(rest) = strip_keyword(line, "requires_field") {
            if let Some(s) = first_symbol(rest) {
                bk.required_fields.push(s);
            }
            continue;
        }

        // trigger_attribute :description
        if let Some(rest) = strip_keyword(line, "trigger_attribute") {
            if let Some(s) = first_symbol(rest) {
                bk.trigger_attribute = Some(s);
            }
            continue;
        }
    }

    if found_header && !bk.name.is_empty() {
        Some(bk)
    } else {
        None
    }
}

// ── tiny line-parsing helpers ───────────────────────────────────────

fn extract_between_quotes(line: &str) -> Option<String> {
    let first = line.find('"')?;
    let rest = &line[first + 1..];
    let second = rest.find('"')?;
    Some(rest[..second].to_string())
}

/// Strip the keyword if the line starts with it followed by whitespace
/// (so `behavior :foo` matches but `behaviors :foo` doesn't, and
/// `trigger_field :foo` matches but `trigger_field_something` doesn't).
fn strip_keyword<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    if !line.starts_with(keyword) {
        return None;
    }
    let after = &line[keyword.len()..];
    let next_byte = after.as_bytes().first()?;
    if matches!(*next_byte, b' ' | b'\t') {
        Some(after.trim_start())
    } else {
        None
    }
}

/// First `:symbol` on the line (sans the leading colon). Comments after
/// the symbol are tolerated.
fn first_symbol(s: &str) -> Option<String> {
    let trimmed = s.trim_start();
    let bytes = trimmed.as_bytes();
    if bytes.first() != Some(&b':') {
        return None;
    }
    let mut i = 1;
    while i < bytes.len() {
        let c = bytes[i];
        if c.is_ascii_alphanumeric() || c == b'_' {
            i += 1;
        } else {
            break;
        }
    }
    if i == 1 {
        None
    } else {
        Some(trimmed[1..i].to_string())
    }
}

/// Collect every `:symbol` on the rest of a line into `out`. Used for
/// `fields :a, :b, :c` and `providers :x, :y`. Stops at a trailing
/// `# comment`.
fn collect_symbols(s: &str, out: &mut Vec<String>) {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Stop at trailing comment.
        if bytes[i] == b'#' {
            break;
        }
        if bytes[i] == b':' {
            // Start of a symbol — skip the colon, collect ident.
            i += 1;
            let start = i;
            while i < bytes.len()
                && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_')
            {
                i += 1;
            }
            if i > start {
                if let Ok(name) = std::str::from_utf8(&bytes[start..i]) {
                    out.push(name.to_string());
                }
            }
        } else {
            i += 1;
        }
    }
}
