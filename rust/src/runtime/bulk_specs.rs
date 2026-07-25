//! bulk_specs — spec-list coercion for many-form bulk dispatch (i113 /
//! i116) : turn a `Value::List` or JSON-encoded string of row specs into
//! per-row attrs hashes. The consuming row loop (`dispatch_bulk`) lives in
//! command_dispatch.rs — this cask is the pure input-coercion leaf.
//!
//! Cask extracted VERBATIM from runtime/command_dispatch.rs (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/bulk_specs.rs — kernel-floor dispatch
//!  input coercion, relocated verbatim from command_dispatch.rs blanket.]

use super::{RuntimeError, Value};
use std::collections::HashMap;

/// Coerce a Value (Map or Str-encoded JSON object) into a per-row
/// attrs hash. Map fields are picked up directly ; a Str body is
/// parsed as JSON when the VO field set is known.
pub(super) fn spec_to_attrs(spec: &Value, vo_fields: &[String]) -> HashMap<String, Value> {
    match spec {
        Value::Map(m) => m.clone(),
        Value::Str(s) => {
            // Best-effort JSON object parse for CLI-passed specs.
            if let Ok(serde_json::Value::Object(map)) = serde_json::from_str::<serde_json::Value>(s) {
                let mut out = HashMap::new();
                for f in vo_fields {
                    if let Some(v) = map.get(f) {
                        out.insert(f.clone(), json_to_value(v));
                    }
                }
                out
            } else {
                HashMap::new()
            }
        }
        _ => HashMap::new(),
    }
}

fn json_to_value(v: &serde_json::Value) -> Value {
    match v {
        serde_json::Value::String(s) => Value::Str(s.clone()),
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else {
                Value::Str(n.to_string())
            }
        }
        serde_json::Value::Null => Value::Null,
        // Recurse : a nested value object stays a Value::Map (arrays a List),
        // not a stringified blob — same contract as repository::from_json.
        serde_json::Value::Array(a) => Value::List(a.iter().map(json_to_value).collect()),
        serde_json::Value::Object(m) => {
            Value::Map(m.iter().map(|(k, v)| (k.clone(), json_to_value(v))).collect())
        }
    }
}

/// Parse a string-encoded specs list — the CLI path. Accepts a JSON
/// array of objects ; each object becomes a Value::Map. Anything else
/// surfaces as an error so the caller sees the shape mismatch instead
/// of silently saving zero rows.
pub(super) fn parse_specs_from_str(s: &str) -> Result<Vec<Value>, RuntimeError> {
    let parsed: serde_json::Value = serde_json::from_str(s)
        .map_err(|e| RuntimeError::MissingAttribute(format!("specs: invalid JSON ({})", e)))?;
    let arr = parsed.as_array().ok_or_else(|| {
        RuntimeError::MissingAttribute("specs: expected JSON array".into())
    })?;
    let mut out = Vec::with_capacity(arr.len());
    for raw in arr {
        if let serde_json::Value::Object(obj) = raw {
            let mut m = HashMap::new();
            for (k, v) in obj {
                m.insert(k.clone(), json_to_value(v));
            }
            out.push(Value::Map(m));
        } else {
            out.push(json_to_value(raw));
        }
    }
    Ok(out)
}
