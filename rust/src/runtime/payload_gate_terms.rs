//! payload_gate_terms — the judgeability grammar of the payload gate :
//! judgeable (the conservative whitelist of &&/|| comparison chains the
//! expression interpreter provably speaks) and simple_term (quoted string
//! / numeric literal / identifier with optional .size/.length). Anything
//! else passes the gate unjudged rather than being refused by
//! incomprehension. The gate loop stays in payload_gate.rs.
//!
//! Cask extracted VERBATIM from runtime/payload_gate.rs (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/payload_gate_terms.rs — kernel-floor
//!  gate grammar, relocated verbatim from payload_gate.rs blanket.]

/// Conservative whitelist of what the expression interpreter provably
/// speaks at the gate : `&&`/`||` chains of comparisons whose terms are
/// quoted strings, numeric literals (int or float), or identifiers
/// (optionally `.size` / `.length`). Anything else — negation, method
/// calls, assignment, blocks — is NOT judgeable and passes the gate
/// unjudged rather than being refused by incomprehension.
pub(super) fn judgeable(expr: &str) -> bool {
    expr.split("&&")
        .flat_map(|p| p.split("||"))
        .all(|clause| {
            let c = clause.trim();
            for op in ["==", "!=", ">=", "<=", ">", "<"] {
                if let Some((l, r)) = c.split_once(op) {
                    return simple_term(l) && simple_term(r);
                }
            }
            false
        })
}

pub(super) fn simple_term(t: &str) -> bool {
    let t = t.trim();
    if t.is_empty() {
        return false;
    }
    if t.starts_with('"') && t.ends_with('"') && t.len() >= 2 {
        return true;
    }
    if t.parse::<f64>().is_ok() {
        return true;
    }
    let base = t
        .strip_suffix(".size")
        .or_else(|| t.strip_suffix(".length"))
        .unwrap_or(t);
    !base.is_empty()
        && base.chars().next().map(|c| c.is_alphabetic() || c == '_').unwrap_or(false)
        && base.chars().all(|c| c.is_alphanumeric() || c == '_')
}
