//! registry_surface — the kernel-hook entry point of the mcp dispatcher :
//! substitute_value / substitute (the `{attr}` placeholder pass, i594) and
//! dispatch_via_registry (the FrameworkRegistry-shaped wrapper). The live
//! session dispatch stays in mod.rs ; old `mcp_dispatcher::` paths hold
//! via re-exports.
//!
//! Cask extracted VERBATIM from mcp_dispatcher/mod.rs (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/mcp_dispatcher/registry_surface.rs —
//!  kernel-floor MCP hook surface, relocated verbatim from mod.rs blanket.]

use super::dispatch;
use crate::runtime::framework_registry::KernelResult;
use std::collections::HashMap;

/// Public alias of `substitute` so the runtime's `resolve_mcp_adapters`
/// arm (i594) can run the same placeholder pass before calling
/// `dispatch`. Kept distinct from the private internal symbol so the
/// kernel-hook path (`dispatch_via_registry`) stays its own surface.
pub fn substitute_value(value: serde_json::Value, attrs: &HashMap<String, String>) -> serde_json::Value {
    substitute(value, attrs)
}

/// Substitute `{attr_name}` placeholders in a JSON value's string
/// fields using `command_attrs`. Walks the JSON tree recursively ;
/// only string values are substituted. Unknown placeholders pass
/// through unchanged (the MCP tool will surface the error).
fn substitute(value: serde_json::Value, attrs: &HashMap<String, String>) -> serde_json::Value {
    match value {
        serde_json::Value::String(s) => {
            let mut out = s;
            for (k, v) in attrs {
                let placeholder = format!("{{{}}}", k);
                if out.contains(&placeholder) {
                    out = out.replace(&placeholder, v);
                }
            }
            serde_json::Value::String(out)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.into_iter().map(|v| substitute(v, attrs)).collect())
        }
        serde_json::Value::Object(obj) => {
            let mut out = serde_json::Map::new();
            for (k, v) in obj {
                out.insert(k, substitute(v, attrs));
            }
            serde_json::Value::Object(out)
        }
        v => v,
    }
}

/// Adapt the MCP dispatcher to the `KernelHook` signature the framework
/// registry expects. `adapter_fields` carries the adapter's declared
/// fields (`server`, `tool`, `args`, `command`, `result_into`) ;
/// `command_attrs` carries the dispatched command's attributes
/// (used for placeholder substitution in `args`).
pub fn dispatch_via_registry(
    adapter_fields: &HashMap<String, String>,
    command_attrs: &HashMap<String, String>,
) -> KernelResult {
    let server = adapter_fields.get("server").cloned().unwrap_or_default();
    let tool = adapter_fields.get("tool").cloned().unwrap_or_default();

    // :args arrives as a JSON-encoded string ; parse, substitute, repass.
    // If the field is missing or empty, pass an empty object so tools
    // that take no args still work.
    let args_raw = adapter_fields.get("args").cloned().unwrap_or_default();
    let args_value: serde_json::Value = if args_raw.is_empty() {
        serde_json::Value::Object(Default::default())
    } else {
        match serde_json::from_str(&args_raw) {
            Ok(v) => v,
            Err(e) => {
                return KernelResult {
                    kind: "mcp".into(),
                    ok: false,
                    output: String::new(),
                    exit_code: 0,
                    error: Some(format!("mcp : args is not JSON : {}", e)),
                };
            }
        }
    };
    let args_substituted = substitute(args_value, command_attrs);

    let r = dispatch(&server, &tool, &args_substituted);
    KernelResult {
        kind: format!("mcp:{}", tool),
        ok: !r.is_error,
        output: if r.text.is_empty() { r.structured.clone() } else { r.text },
        exit_code: 0,
        error: if r.error.is_empty() { None } else { Some(r.error) },
    }
}
