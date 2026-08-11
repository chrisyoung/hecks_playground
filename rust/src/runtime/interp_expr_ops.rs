//! interp_expr_ops — the expression grammar's leaf operations : top-level
//! splitting that respects nesting and quotes (split_call_args,
//! split_top_level, split_comparison), the Value comparison semantics
//! (values_equal, compare_lt, numeric_value), and the deterministic
//! xorshift rand_below_impl. Pure functions ; the resolver that composes
//! them lives in interpreter.rs.
//!
//! Cask extracted VERBATIM from runtime/interpreter.rs (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/interp_expr_ops.rs — kernel-floor
//!  interpreter leaves, relocated verbatim from interpreter.rs blanket.]

use super::Value;

/// Split a call's argument list on top-level commas — ignoring commas inside
/// nested parens or quoted strings. An empty / whitespace list yields an empty
/// vec. Used to bind a VO derivation's positional params at a method call.
pub(super) fn split_call_args(s: &str) -> Vec<String> {
    let s = s.trim();
    if s.is_empty() {
        return Vec::new();
    }
    let mut args = Vec::new();
    let mut depth = 0i32;
    let mut in_str = false;
    let mut start = 0;
    let bytes = s.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'"' => in_str = !in_str,
            b'(' if !in_str => depth += 1,
            b')' if !in_str => depth -= 1,
            b',' if !in_str && depth == 0 => {
                args.push(s[start..i].trim().to_string());
                start = i + 1;
            }
            _ => {}
        }
    }
    args.push(s[start..].trim().to_string());
    args
}

/// Split on the first occurrence of `op` that isn't inside a quoted
/// string. Returns (lhs, rhs) trimmed slices, or None if not found.
/// Used for boolean operators `||` and `&&` where quoted comparison
/// values may legitimately contain pipe/ampersand characters.
pub(super) fn split_top_level<'a>(expr: &'a str, op: &str) -> Option<(&'a str, &'a str)> {
    let bytes = expr.as_bytes();
    let op_bytes = op.as_bytes();
    let mut in_str = false;
    let mut i = 0;
    while i + op_bytes.len() <= bytes.len() {
        let b = bytes[i];
        if b == b'"' { in_str = !in_str; }
        if !in_str && &bytes[i..i + op_bytes.len()] == op_bytes {
            let lhs = expr[..i].trim();
            let rhs = expr[i + op_bytes.len()..].trim();
            return Some((lhs, rhs));
        }
        i += 1;
    }
    None
}

pub(super) fn split_comparison<'a>(expr: &'a str, op: &str) -> Option<(&'a str, &'a str)> {
    // Disambiguate single-char ops from their multi-char counterparts.
    // `<` must not match against `<=` or `<` inside `!=`/`==`/`>=`.
    if op == "<" && (expr.contains("<=") || expr.contains("<<")) {
        return None;
    }
    if op == ">" && (expr.contains(">=") || expr.contains(">>")) {
        return None;
    }
    if op == "=" {
        return None; // Bare `=` isn't a comparison; force callers to use ==.
    }
    let idx = expr.find(op)?;
    // For ==/!=, ensure we're not confusing them with =/= prefixes; find
    // returns the first occurrence of the literal substring so this is OK.
    Some((&expr[..idx], &expr[idx + op.len()..]))
}

/// Loose equality: Bool(true) == Str("true"), Int(42) == Str("42").
/// Numeric coercion lets `Int(0) == Str("0")`, `Int(0) == Null` (Null
/// stands in for an unset numeric attr), and `Str("1.0") == Int(1)`
/// (Float values stored as strings still compare to whole-number ints).
pub(super) fn values_equal(left: &Value, right: &Value) -> bool {
    if left == right {
        return true;
    }
    if let (Some(a), Some(b)) = (numeric_value(left), numeric_value(right)) {
        return a == b;
    }
    let ls = format!("{}", left);
    let rs = format!("{}", right);
    ls == rs
}

pub(super) fn compare_lt(left: &Value, right: &Value) -> bool {
    // Try Int<Int first; fall back to numeric coercion through f64 so
    // Float-valued attrs (stored as Str("1.0")) and mixed Int/Str
    // comparisons (e.g. `amount > 0` where amount is Str("1.5")) work.
    if let (Value::Int(a), Value::Int(b)) = (left, right) { return a < b; }
    let l = numeric_value(left);
    let r = numeric_value(right);
    match (l, r) {
        (Some(a), Some(b)) => a < b,
        _ => false,
    }
}

/// Best-effort numeric coercion: Int as-is, Str parsed as f64, Bool to
/// 0/1, Null as 0 (an unset Integer attr behaves as zero). Returns None
/// for List/Map and unparseable strings.
pub(super) fn numeric_value(v: &Value) -> Option<f64> {
    match v {
        Value::Int(n) => Some(*n as f64),
        Value::Bool(true) => Some(1.0),
        Value::Bool(false) => Some(0.0),
        Value::Str(s) => s.parse::<f64>().ok(),
        Value::Null => Some(0.0),
        _ => None,
    }
}

/// Uniform random integer in [0, n). Backed by a thread-local LCG
/// seeded from system nanos + a per-call counter — no rand crate
/// dependency, deterministic-enough for stochastic dispatch gates
/// (the bluebook just needs ~uniform distribution over many calls,
/// not cryptographic strength). For tests, set HECKS_RAND_SEED=0 to
/// always return 0 (the predicate fires every call) ; HECKS_RAND_SEED
/// to any other positive integer makes rand_below always return that
/// value mod n (deterministic for fixture replay).
pub(super) fn rand_below_impl(n: i64) -> i64 {
    if let Ok(seed) = std::env::var("HECKS_RAND_SEED") {
        if let Ok(s) = seed.parse::<i64>() {
            return s.rem_euclid(n);
        }
    }
    use std::cell::Cell;
    thread_local! {
        static STATE: Cell<u64> = const { Cell::new(0) };
    }
    STATE.with(|s| {
        let mut x = s.get();
        if x == 0 {
            // wasm-safe clock : raw std::time::SystemTime::now()
            // panics "time not implemented" on wasm32 (CF Worker).
            // Route the xorshift seed through clock::now_duration
            // (i7, the i630 follow-up). `| 1` keeps the non-zero
            // invariant the xorshift needs.
            x = (crate::clock::now_duration().as_nanos() as u64) | 1;
        }
        // xorshift64 — fast, decent distribution, no dependency
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        s.set(x);
        (x % (n as u64)) as i64
    })
}
