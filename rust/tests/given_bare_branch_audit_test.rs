//! given_bare_branch_audit — the fails-open audit for the `given` gate.
//!
//! `evaluate_given` ends in a bare arm : resolve the expression, honour a Bool,
//! and pass ANYTHING else (`_ => true`, "the historical permissive default").
//! Every typo, every renamed attribute, every operator the floor does not speak
//! lands there and reads as a satisfied rule — and under a leading `!` the same
//! default inverts into a permanent refusal. A rule that cannot be made to fail
//! is not a rule ; one that cannot be made to pass is not one either.
//!
//! So this walks every `.bluebook` in the tree, finds each `given` /
//! `holds_when` clause that reaches the bare arm, and REFUSES any that is not a
//! declared `Boolean` derivation — the one shape the arm can actually judge
//! (`given { balance.covers?(amount) }`, backed by
//! `derive :covers?, Boolean do |other| ... end` on the receiver's value object).
//!
//! A new bare given that no VO declares fails here, at test time, rather than
//! silently at dispatch time. Run with `--nocapture` to read the admitted list.

use std::fs;
use std::path::{Path, PathBuf};
use storehouse::ir::{Aggregate, Command};
use storehouse::parser;

fn hecks_root() -> PathBuf {
    std::env::var("HECKS_CONCEPTION_DIR")
        .ok()
        .and_then(|c| Path::new(&c).parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| {
            let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let in_tree = manifest.join("..");
            if in_tree.join("hecks_conception").is_dir() {
                in_tree
            } else {
                manifest.join("../../hecks")
            }
        })
}

fn collect_bluebooks(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') || name == "target" || name == "node_modules" {
            continue;
        }
        if path.is_dir() {
            collect_bluebooks(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("bluebook") {
            out.push(path);
        }
    }
}

/// Mirrors `evaluate_given`'s branch order : an expression reaches the final
/// bare arm only when no earlier branch claims it. `||` and `&&` RECURSE in the
/// evaluator, so a compound expression is not itself a leaf — each clause is
/// classified on its own, or every bare clause sitting beside a comparison
/// would hide. Deliberately conservative elsewhere (plain substring tests), so
/// this over-reports rather than missing a site.
fn bare_clauses(expr: &str, out: &mut Vec<String>) {
    let e = expr.trim();
    for op in ["||", "&&"] {
        if let Some((lhs, rhs)) = e.split_once(op) {
            bare_clauses(lhs, out);
            bare_clauses(rhs, out);
            return;
        }
    }
    if let Some(inner) = e.strip_prefix('!') {
        bare_clauses(inner, out);
        return;
    }
    for suffix in [".any?", ".empty?", ".nil?"] {
        if e.ends_with(suffix) {
            return;
        }
    }
    if e.contains(".include?(") && e.ends_with(')') {
        return;
    }
    for op in [">=", "<=", "==", "!=", "<", ">"] {
        if e.contains(op) {
            return;
        }
    }
    out.push(e.to_string());
}

/// `amount.same_currency?(balance)` -> ("amount", "same_currency?").
fn split_receiver_method(clause: &str) -> Option<(&str, &str)> {
    let head = match clause.find('(') {
        Some(open) if clause.ends_with(')') => &clause[..open],
        _ => clause,
    };
    let dot = head.rfind('.')?;
    let receiver = head[..dot].trim();
    let method = head[dot + 1..].trim();
    if receiver.is_empty() || receiver.contains('.') || method.is_empty() {
        return None;
    }
    Some((receiver, method))
}

/// The one judgeable bare shape : the receiver is an attribute whose value
/// object declares `derive :method, Boolean`. Anything else resolves to a
/// non-Bool and is decided by the permissive default rather than by the author.
fn names_a_boolean_derivation(agg: &Aggregate, cmd: Option<&Command>, clause: &str) -> bool {
    let Some((receiver, method)) = split_receiver_method(clause) else {
        return false;
    };
    let declared_type = cmd
        .and_then(|c| c.attributes.iter().find(|a| a.name == receiver))
        .or_else(|| agg.attributes.iter().find(|a| a.name == receiver))
        .map(|a| a.attr_type.clone());
    let Some(vo_type) = declared_type else { return false };
    agg.value_objects
        .iter()
        .filter(|vo| vo.name == vo_type)
        .any(|vo| {
            vo.derivations
                .iter()
                .any(|d| d.name == method && d.return_type == "Boolean")
        })
}

#[test]
fn every_bare_given_names_a_boolean_derivation() {
    let root = hecks_root();
    let mut files = Vec::new();
    collect_bluebooks(&root, &mut files);
    assert!(!files.is_empty(), "no .bluebook files found under {root:?}");
    files.sort();

    let mut admitted: Vec<String> = Vec::new();
    let mut unjudgeable: Vec<String> = Vec::new();

    for file in &files {
        let Ok(src) = fs::read_to_string(file) else { continue };
        let domain = parser::parse(&src);
        let rel = file.strip_prefix(&root).unwrap_or(file).display().to_string();
        for agg in &domain.aggregates {
            let mut sites: Vec<(String, Option<&Command>, String)> = Vec::new();
            for cmd in &agg.commands {
                for given in &cmd.givens {
                    sites.push((
                        format!("{}.{}", agg.name, cmd.name),
                        Some(cmd),
                        given.expression.clone(),
                    ));
                }
            }
            for inv in &agg.invariants {
                sites.push((
                    format!("{} holds_when {:?}", agg.name, inv.name),
                    None,
                    inv.expression.clone(),
                ));
            }
            for (where_, cmd, expression) in sites {
                let mut clauses = Vec::new();
                bare_clauses(&expression, &mut clauses);
                for clause in clauses {
                    let line = format!("{rel}\n    {where_}  {{ {expression} }}\n      clause: {clause}");
                    if names_a_boolean_derivation(agg, cmd, &clause) {
                        admitted.push(line);
                    } else {
                        unjudgeable.push(line);
                    }
                }
            }
        }
    }

    println!(
        "\n=== bare-arm givens across {} bluebooks : {} judgeable, {} not ===",
        files.len(),
        admitted.len(),
        unjudgeable.len()
    );
    for a in &admitted {
        println!("  [derivation] {a}");
    }

    assert!(
        unjudgeable.is_empty(),
        "these givens reach the bare arm and name no Boolean derivation, so the \
         permissive default decides them — they pass whatever the author meant, and \
         invert to a permanent refusal under `!` :\n\n{}\n",
        unjudgeable.join("\n")
    );
}
