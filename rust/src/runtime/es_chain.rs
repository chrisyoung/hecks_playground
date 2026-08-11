//! es_chain — the hash-chain integrity of the event Log : deterministic
//! canonicalisation (canon_value — VO round-trip + sorted map keys, so the
//! verifier never cries wolf), the per-entry content hash + prev_hash
//! stamping (entry_content_hash, stamp_chain), and the chain_breaks
//! verifier. Detects corruption / truncation / reordering ; NOT proof
//! against deliberate rewrite (see the fn docs). Inherent `impl Runtime`
//! methods in a child module, same contract as event_sourcing.rs.
//!
//! Cask extracted VERBATIM from runtime/event_sourcing.rs (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/es_chain.rs — kernel-floor runtime,
//!  relocated verbatim from event_sourcing.rs blanket.]

use super::*;
use std::collections::HashMap;

impl Runtime {
    /// TRUST (Stage 5) — the content hash of one Log entry, over its own fields
    /// AND the previous entry's hash. Chaining is what turns a per-row checksum
    /// into evidence about the SEQUENCE : change one entry and every entry after
    /// it stops re-deriving.
    ///
    /// Canonical by construction : fields are sorted by name and the hash fields
    /// themselves are excluded, so the digest depends on the entry's MEANING and
    /// not on map iteration order. Values go through `value_to_json_string`, the
    /// same faithful serialisation the Log stores — so a Money map contributes its
    /// structure, not "{2 fields}".
    ///
    /// THE DIGEST IS FNV-1a-64, AND THAT IS A DELIBERATE, NAMED LIMIT. It detects
    /// corruption, truncation and reordering — a log that was supposed to be
    /// immutable and quietly is not. It is NOT proof against a deliberate rewrite:
    /// anyone who can edit an entry can recompute the chain after it. Real
    /// tamper-evidence needs a cryptographic digest AND an anchor the rewriter
    /// cannot reach (a signed or externally-witnessed head) — and a crypto digest
    /// alone, with the head still local, would only LOOK stronger. The chain is
    /// the mechanism ; swapping FNV for SHA-256 is one function, once this lean
    /// crate is willing to take the dependency.
    /// One field's contribution to the digest, normalised so the SAME entry hashes
    /// the same whether it is the attrs going INTO Append or the record coming back
    /// OUT of the store. A single-attribute value object round-trips as either the
    /// bare scalar or `{value: scalar}` depending on which side you are on ; without
    /// this every entry would fail to re-derive and the chain would cry wolf on a
    /// log nobody touched.
    /// It ALSO sorts nested map keys. `Value::Map` is a HashMap, so the same
    /// content serialises as `{verb,inputs}` on one entry and `{inputs,verb}` on
    /// the next — hashing that raw would make the digest depend on iteration order
    /// and the chain would report breaks on a log nobody touched. Determinism is
    /// not a nicety here : a verifier that cries wolf is worse than none.
    fn canon_value(v: &Value) -> String {
        match v {
            // A single-attribute value object : unwrap, so the bare scalar and
            // `{value: scalar}` forms agree.
            Value::Map(m) if m.len() == 1 && m.contains_key("value") => {
                Self::canon_value(m.get("value").expect("checked"))
            }
            Value::Map(m) => {
                let mut keys: Vec<&String> = m.keys().collect();
                keys.sort();
                let mut s = String::from("{");
                for k in keys {
                    s.push_str(k);
                    s.push(':');
                    if let Some(inner) = m.get(k) {
                        s.push_str(&Self::canon_value(inner));
                    }
                    s.push(',');
                }
                s.push('}');
                s
            }
            // Lists keep their order — it is content, not incidental.
            Value::List(items) => {
                let mut s = String::from("[");
                for it in items {
                    s.push_str(&Self::canon_value(it));
                    s.push(',');
                }
                s.push(']');
                s
            }
            other => super::value_to_json_string(other),
        }
    }

    /// TRUST : every entry whose hash does not re-derive. Empty means intact.
    /// Grouped per SHARD (the event_id prefix before its last `-`), ordered by
    /// sequence within the shard — the order the single writer appended them.
    pub(crate) fn chain_breaks(&self, agg_name: &str) -> Vec<serde_json::Value> {
        let mut by_shard: HashMap<String, Vec<&AggregateState>> = HashMap::new();
        for ev in self.all(agg_name) {
            let id = match ev.get("event_id") {
                Value::Str(s) => s.clone(),
                Value::Map(m) => m.get("value").map(|v| v.to_string()).unwrap_or_default(),
                _ => continue,
            };
            if let Some((shard, _)) = id.rsplit_once('-') {
                by_shard.entry(shard.to_string()).or_default().push(ev);
            }
        }
        let field_str = |ev: &AggregateState, k: &str| -> String {
            match ev.get(k) {
                Value::Str(s) => s.clone(),
                Value::Map(m) => m.get("value").map(|v| v.to_string()).unwrap_or_default(),
                _ => String::new(),
            }
        };
        let mut shards: Vec<String> = by_shard.keys().cloned().collect();
        shards.sort();
        let mut breaks = Vec::new();
        for shard in shards {
            let mut entries = by_shard.remove(&shard).unwrap_or_default();
            entries.sort_by_key(|e| match e.get("sequence") {
                Value::Int(i) => *i,
                Value::Map(m) => m.get("value").and_then(|v| v.as_int()).unwrap_or(0),
                _ => 0,
            });
            let mut expected_prev = String::new();
            for ev in entries {
                let recorded = field_str(ev, "entry_hash");
                let prev = field_str(ev, "prev_hash");
                let id = field_str(ev, "event_id");
                // LINK : does this entry name the one before it?
                if prev != expected_prev {
                    breaks.push(serde_json::json!({
                        "event_id": id, "shard": shard, "break": "link",
                        "expected_prev": expected_prev, "found_prev": prev,
                    }));
                }
                // CONTENT : does the entry still hash to what it claims?
                let derived = Self::entry_content_hash(&ev.fields, &prev);
                if derived != recorded {
                    breaks.push(serde_json::json!({
                        "event_id": id, "shard": shard, "break": "content",
                        "recorded": recorded, "derived": derived,
                    }));
                }
                expected_prev = recorded;
            }
        }
        breaks
    }

    fn entry_content_hash(attrs: &HashMap<String, Value>, prev_hash: &str) -> String {
        let mut keys: Vec<&String> = attrs
            .keys()
            .filter(|k| k.as_str() != "prev_hash" && k.as_str() != "entry_hash")
            .collect();
        keys.sort();
        let mut buf = String::new();
        for k in keys {
            buf.push_str(k);
            buf.push('=');
            if let Some(v) = attrs.get(k) {
                buf.push_str(&Self::canon_value(v));
            }
            buf.push(';');
        }
        buf.push_str("prev=");
        buf.push_str(prev_hash);

        // FNV-1a-64.
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in buf.as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        format!("{:016x}", h)
    }

    /// Stamp `prev_hash` + `entry_hash` onto an entry about to be appended, and
    /// advance this process's chain head. The chain is PER SHARD and a shard has
    /// exactly one writer — this process — so the head is simply the last entry
    /// this runtime wrote. Empty prev_hash marks the first entry of a chain, where
    /// a walk starts.
    pub(super) fn stamp_chain(&mut self, attrs: &mut HashMap<String, Value>) {
        let prev = self.chain_head.clone().unwrap_or_default();
        let hash = Self::entry_content_hash(attrs, &prev);
        attrs.insert("prev_hash".to_string(), Value::Str(prev));
        attrs.insert("entry_hash".to_string(), Value::Str(hash.clone()));
        self.chain_head = Some(hash);
    }
}
