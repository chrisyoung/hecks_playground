//! query_event_fields — the Event-record field readers the query
//! interpreter leans on : id_field (plain string OR {value} VO — the Log
//! stamps VOs, fixtures carry bare strings), seq_field, record_json (the
//! full-record JSON map). The interpreter itself stays in query.rs.
//!
//! Cask extracted VERBATIM from runtime/query.rs (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/query_event_fields.rs — kernel-floor
//!  query field readers, relocated verbatim from query.rs blanket.]

use super::value_convert::value_to_json;
use super::{AggregateState, Value};

/// Read an id-shaped Event field that may be a plain string OR a `{value}` VO.
/// The Log stamps VOs ; hand-constructed fixtures often carry bare strings, and
/// both lineage traversals must read either.
pub(super) fn id_field(ev: &AggregateState, key: &str) -> String {
    match ev.fields.get(key) {
        Some(Value::Str(s)) => s.clone(),
        Some(Value::Map(m)) => m.get("value").map(|v| v.to_string()).unwrap_or_default(),
        _ => String::new(),
    }
}

/// The Log sequence of an event (0 when absent) — the total order the forward
/// walk sorts siblings by, so a consequence tree is deterministic.
pub(super) fn seq_field(ev: &AggregateState) -> i64 {
    match ev.fields.get("sequence") {
        Some(Value::Int(i)) => *i,
        Some(Value::Map(m)) => m.get("value").and_then(|v| v.as_int()).unwrap_or(0),
        _ => 0,
    }
}

/// One Event as flat JSON — the row shape BOTH lineage traversals return.
///
/// Serialised through `value_to_json`, NOT a match that stringifies the
/// non-scalar arms. The old form rendered every value object as the useless
/// `"{N fields}"` — so a lineage row lost its `event_name`, `sequence`, `delta`
/// and `verdict` wholesale, which is exactly the lossiness the Log itself was
/// fixed for (the source-of-truth floor). A traversal that reports the Log must
/// report it as faithfully as the Log stores it.
pub(super) fn record_json(ev: &AggregateState) -> serde_json::Map<String, serde_json::Value> {
    let mut map = serde_json::Map::new();
    for (k, v) in &ev.fields {
        map.insert(k.clone(), value_to_json(v));
    }
    map
}
