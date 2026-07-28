//! value_convert — Value <-> JSON conversion + coercion, the runtime's one
//! value-shape boundary. Everything that turns a runtime `Value` into JSON
//! (value_to_json / value_to_json_string), JSON back into a typed Value
//! (value_from_json_str / json_to_value_recursive / json_obj_to_value_map /
//! parse_payload_attrs), a raw `k=v` dispatch-arg string into a Value
//! (attr_value_from_str), plus Value's own coercion surface (as_str / as_int
//! / as_bool / as_list) and its lossy Display. Pure functions of their
//! inputs — no runtime state.
//!
//! Cask extracted VERBATIM from runtime/mod.rs (shrink-mod phase B, wave 1) ;
//! the `Value` enum itself stays in mod.rs (the module root owns its types,
//! this cask owns their conversions).
//!
//! [antibody-exempt: rust/src/runtime/value_convert.rs — pure-utility value
//!  plumbing (no domain) : the Value<->JSON converters + coercions the whole
//!  runtime shares, relocated verbatim from mod.rs's blanket exemption.]

use super::*;

/// Map a runtime `Value` into a serde_json value for the rich block.
/// C2 (transactional outbox) — decode a CascadeRun Step's JSON payload
/// back into a dispatch attr map, so the pump can re-dispatch the reaction
/// with the triggering event's data. Inverse of the value_to_json encode in
/// record_cascade_run. Non-object / malformed payloads yield empty attrs.
pub(super) fn parse_payload_attrs(json: &str) -> HashMap<String, Value> {
    let mut out = HashMap::new();
    if let Ok(serde_json::Value::Object(obj)) = serde_json::from_str::<serde_json::Value>(json) {
        for (k, v) in obj {
            let val = match v {
                serde_json::Value::String(s) => Value::Str(s),
                serde_json::Value::Bool(b) => Value::Bool(b),
                serde_json::Value::Number(n) => n.as_i64().map(Value::Int).unwrap_or(Value::Null),
                serde_json::Value::Null => Value::Null,
                other => Value::Str(other.to_string()),
            };
            out.insert(k, val);
        }
    }
    out
}


pub(crate) fn value_to_json(v: &Value) -> serde_json::Value {
    match v {
        Value::Str(s) => serde_json::json!(s),
        Value::Int(n) => serde_json::json!(n),
        Value::Bool(b) => serde_json::json!(b),
        Value::List(items) => serde_json::Value::Array(items.iter().map(value_to_json).collect()),
        Value::Map(m) => {
            let mut o = serde_json::Map::new();
            for (k, val) in m {
                o.insert(k.clone(), value_to_json(val));
            }
            serde_json::Value::Object(o)
        }
        Value::Null => serde_json::Value::Null,
    }
}

/// Inverse of value_to_json : decode a serde_json value back into a runtime
/// `Value`, RECURSIVELY (nested objects -> Value::Map, arrays -> Value::List).
/// Used by unconsolidated_log_tail to rebuild each shard Event's nested shape
/// (delta{field,value}, sequence{value}) so the fold reads tail records exactly
/// as it reads consolidated ones. A whole-number JSON number decodes to Int ;
/// any other number (float) falls back to its string form, matching the heki
/// reader's own from_json.
/// Serialize a runtime Value to a COMPACT JSON string — the faithful,
/// round-trippable form for stashing a value inside a String field (an event-log
/// delta, a serialized payload). Unlike `Value`'s Display, which renders a Map as
/// `"{N fields}"` and a List as `"[N items]"` (LOSSY — the cents of a Money are
/// gone), this preserves structure recursively. `value_from_json_str` is its exact
/// inverse, so a Money / a ledger survives a store-and-reconstruct round-trip.
pub fn value_to_json_string(v: &Value) -> String {
    value_to_json(v).to_string()
}

/// Decode a JSON string produced by `value_to_json_string` back into a typed
/// Value (object -> Map, array -> List, recursively ; whole-number -> Int). Falls
/// back to `Str` for a non-JSON string (a legacy Display-form delta, or a plain
/// word) rather than erroring — so an old lossy delta reads as its stored string
/// and DRIFTS loudly against a structured live value, exactly as it should.
pub fn value_from_json_str(s: &str) -> Value {
    match serde_json::from_str::<serde_json::Value>(s) {
        Ok(jv) => json_to_value_recursive(&jv),
        Err(_) => Value::Str(s.to_string()),
    }
}

/// Parse a raw `k=v` dispatch-arg string into a runtime Value. A JSON object
/// (`{…}`) or array (`[…]`) is decoded STRUCTURALLY — so a nested value object
/// (a Money `{"cents":5000,"currency":{"code":"USD"}}`) arrives as a Map, not a
/// flat string the arithmetic + given evaluators can't read. Everything else
/// stays a Str ; the runtime coerces scalar types per the attribute schema. A
/// malformed `{`/`[` value falls back to Str rather than erroring — a bad arg
/// should reach the payload gate, not break the parse. This is the ONE place
/// every string-keyed dispatch door (the CLI via embed, the warm serve socket)
/// turns an arg string into a Value, so nested-object inputs stop being mangled
/// to `"[object Object]"` / raw JSON text at the tool boundary.
pub fn attr_value_from_str(raw: &str) -> Value {
    let t = raw.trim();
    if (t.starts_with('{') && t.ends_with('}')) || (t.starts_with('[') && t.ends_with(']')) {
        if let Ok(jv) = serde_json::from_str::<serde_json::Value>(t) {
            return json_to_value_recursive(&jv);
        }
        // A `.behaviors` file is RUBY, so a value object is written the way
        // Ruby writes a hash — `{ cents: 1400, currency: "USD" }` — which is not
        // JSON (bare keys, `=>`). Only strict JSON was accepted, so every value
        // object handed to a behaviour arrived as a STRING and every VO-typed
        // given (`amount.positive?`, `balance.covers?(amount)`) had no map to
        // read. That is the whole reason banking ran differently in the two
        // runtimes : Ruby's permissive default admitted the unreadable guard,
        // Rust refused it.
        if let Some(jv) = ruby_hash_to_json(t) {
            return json_to_value_recursive(&jv);
        }
    }
    Value::Str(raw.to_string())
}

/// Ruby hash literal -> JSON. Quotes bare `key:` labels and rewrites `=>`,
/// leaving quoted spans alone so a value containing a colon or brace survives.
fn ruby_hash_to_json(src: &str) -> Option<serde_json::Value> {
    let mut out = String::with_capacity(src.len() + 16);
    let mut chars = src.char_indices().peekable();
    let mut in_string = false;
    while let Some((i, c)) = chars.next() {
        if c == '"' && !src[..i].ends_with('\\') {
            in_string = !in_string;
            out.push(c);
            continue;
        }
        if in_string {
            out.push(c);
            continue;
        }
        // `key:` at the start of a pair becomes `"key":`
        if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            let mut end = i + c.len_utf8();
            while let Some(&(j, n)) = chars.peek() {
                if n.is_alphanumeric() || n == '_' {
                    end = j + n.len_utf8();
                    chars.next();
                } else {
                    break;
                }
            }
            let word = &src[start..end];
            let is_label = matches!(chars.peek(), Some(&(_, ':')));
            if is_label {
                out.push('"');
                out.push_str(word);
                out.push('"');
            } else {
                // a bare token that is not a label — true/false/null pass through,
                // anything else becomes a string so the parse still succeeds.
                match word {
                    "true" | "false" | "null" | "nil" => {
                        out.push_str(if word == "nil" { "null" } else { word })
                    }
                    _ => {
                        out.push('"');
                        out.push_str(word);
                        out.push('"');
                    }
                }
            }
            continue;
        }
        if c == '=' && matches!(chars.peek(), Some(&(_, '>'))) {
            chars.next();
            out.push(':');
            continue;
        }
        out.push(c);
    }
    serde_json::from_str(&out).ok()
}

pub(super) fn json_to_value_recursive(v: &serde_json::Value) -> Value {
    match v {
        serde_json::Value::String(s) => Value::Str(s.clone()),
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) => match n.as_i64() {
            Some(i) => Value::Int(i),
            None => Value::Str(n.to_string()),
        },
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Array(a) => {
            Value::List(a.iter().map(json_to_value_recursive).collect())
        }
        serde_json::Value::Object(m) => {
            Value::Map(m.iter().map(|(k, v)| (k.clone(), json_to_value_recursive(v))).collect())
        }
    }
}

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&Vec<Value>> {
        match self {
            Value::List(v) => Some(v),
            _ => None,
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Str(s) => write!(f, "{}", s),
            Value::Int(n) => write!(f, "{}", n),
            Value::Bool(b) => write!(f, "{}", b),
            Value::List(v) => write!(f, "[{} items]", v.len()),
            Value::Map(m) => write!(f, "{{{} fields}}", m.len()),
            Value::Null => write!(f, "null"),
        }
    }
}


pub(super) fn json_obj_to_value_map(map: serde_json::Map<String, serde_json::Value>) -> HashMap<String, Value> {
    let mut out = HashMap::new();
    for (k, v) in map {
        let value = match v {
            serde_json::Value::String(s) => Value::Str(s),
            serde_json::Value::Bool(b)   => Value::Bool(b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() { Value::Int(i) } else { Value::Str(n.to_string()) }
            }
            serde_json::Value::Null      => Value::Null,
            other                        => Value::Str(other.to_string()),
        };
        out.insert(k, value);
    }
    out
}
