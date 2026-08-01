// attr_decode.rs — turn a dispatch arg STRING into a runtime Value, with the
// attribute's DECLARED SHAPE in hand.
//
// `runtime::attr_value_from_str` has to GUESS. Text that looks like a JSON
// object or array decodes structurally, because a nested value object
// (`daily_limit={"cents":5000,"currency":{"code":"USD"}}`) must arrive as a Map
// or no given can read it. That guess is right for a shape and wrong for a
// DOCUMENT, and it was wrong in the worst possible way for the file tools:
//
//   storehouse ... Tools::FileTool.Write file_path=/x/y.json content='{"a":1}'
//
// `content` is declared `Content`, a one-field text holder — but its VALUE
// looked like JSON, so it decoded to a Map. The claude_tool boundary then
// rendered that Map with `Display`, which summarises a Map as `{N fields}`, and
// `fs::write` put those literal bytes on disk. Writing any JSON file through the
// door silently replaced it with `{3 fields}`, and the tool cheerfully reported
// "wrote 10 bytes". Same class as the lossy-delta bug event_sourced_faithful
// _delta_test pins — a Display form standing in for a value — one door over.
//
// THE DECLARED TYPE SETTLES IT, so nothing has to guess:
//
//   String, or a value object with ONE String field   TEXT — keep it verbatim
//   anything else (Money { cents, currency })         SHAPE — decode structurally
//
// A one-field text holder has no nesting to decode INTO, so whatever the caller
// wrote IS the value. This is the same rule Ruby's `Value.scalar` reads a
// one-field value object by, said at the door instead of after it.

use crate::runtime::Value;
use std::collections::{HashMap, HashSet};

/// The attrs of `command` whose declared type carries TEXT rather than structure.
///
/// Unioned across every same-named command, exactly as `command_attrs` unions
/// allowed keys, so a homonym in another realm cannot change how an arg decodes.
/// An unresolvable command yields an empty set — every arg then decodes as it
/// always did, and UnknownCommand is reported downstream as before.
pub fn text_typed_attrs(domain: &crate::ir::Domain, command: &str) -> HashSet<String> {
    let mut text = HashSet::new();
    let Some((head, cmd_name)) = command.rsplit_once('.') else {
        return text;
    };
    let agg_name = head.rsplit("::").next().unwrap_or(head);
    for agg in domain.aggregates.iter().filter(|a| a.name == agg_name) {
        for cmd in agg.commands.iter().filter(|c| c.name == cmd_name) {
            for a in cmd.attributes.iter().filter(|a| !a.list) {
                if is_text_type(agg, &a.attr_type) {
                    text.insert(a.name.clone());
                }
            }
        }
    }
    text
}

/// A plain String, or a value object that is one String field and nothing else.
fn is_text_type(agg: &crate::ir::Aggregate, attr_type: &str) -> bool {
    if attr_type == "String" {
        return true;
    }
    match agg.value_objects.iter().find(|v| v.name == attr_type) {
        Some(vo) => {
            vo.attributes.len() == 1
                && !vo.attributes[0].list
                && vo.attributes[0].attr_type == "String"
        }
        None => false,
    }
}

/// Decode a whole raw attr map, honouring each attribute's declared shape.
///
/// This is what every string-keyed dispatch door should call instead of mapping
/// `attr_value_from_str` over the values blind.
pub fn decode_attrs(
    domain: &crate::ir::Domain,
    command: &str,
    attrs: HashMap<String, String>,
) -> HashMap<String, Value> {
    let text = text_typed_attrs(domain, command);
    attrs
        .into_iter()
        .map(|(k, v)| {
            let value = if text.contains(&k) {
                Value::Str(v)
            } else {
                crate::runtime::attr_value_from_str(&v)
            };
            (k, value)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = r#"
Hecks.bluebook "Probe" do
  aggregate "Vault" do
    description "A probe with one text holder and one real shape."
    identified_by :id
    attribute :id, String

    value_object "Content" do
      attribute :value, String
    end

    value_object "Money" do
      attribute :cents, Integer
      attribute :currency, String
    end

    command "Store" do
      attribute :id, String
      attribute :content, Content
      attribute :balance, Money
      emits "Stored"
    end
  end
end
"#;

    fn decoded(content: &str) -> HashMap<String, Value> {
        let domain = crate::parser::parse(SRC);
        let mut attrs = HashMap::new();
        attrs.insert("content".to_string(), content.to_string());
        attrs.insert(
            "balance".to_string(),
            r#"{"cents":5000,"currency":"USD"}"#.to_string(),
        );
        decode_attrs(&domain, "Probe::Vault.Store", attrs)
    }

    /// THE BUG. A one-field text holder whose VALUE happens to look like JSON is
    /// a document, not a shape. Decoding it structurally lost the text, and the
    /// tool boundary then wrote `{N fields}` over the file.
    #[test]
    fn json_shaped_text_survives_verbatim() {
        let document = "{\n  \"name\": \"kitchen\",\n  \"steps\": [1, 2]\n}";
        let out = decoded(document);

        assert_eq!(
            out.get("content"),
            Some(&Value::Str(document.to_string())),
            "content is declared Content — one String field — so it is TEXT, \
             byte for byte, indentation and key order included"
        );
    }

    /// AND THE REASON THE GUESS EXISTS, still working: a genuinely nested value
    /// object has to arrive as a Map or no given can read it.
    #[test]
    fn a_real_value_object_still_decodes_structurally() {
        let out = decoded("plain text");
        let Some(Value::Map(balance)) = out.get("balance") else {
            panic!("balance must decode to a Map, got {:?}", out.get("balance"));
        };
        assert_eq!(balance.get("cents"), Some(&Value::Int(5000)));
    }

    /// Ordinary text is untouched either way.
    #[test]
    fn ordinary_text_is_unchanged() {
        let out = decoded("just a sentence");
        assert_eq!(out.get("content"), Some(&Value::Str("just a sentence".into())));
    }

    /// An unresolvable command decodes exactly as it always did, so a wrong verb
    /// reaches UnknownCommand downstream rather than changing how args parse.
    #[test]
    fn an_unknown_command_names_no_text_attrs() {
        let domain = crate::parser::parse(SRC);
        assert!(text_typed_attrs(&domain, "Probe::Vault.Nonesuch").is_empty());
        assert!(text_typed_attrs(&domain, "not-a-command").is_empty());
    }
}
