//! [antibody-exempt: rust/src/runtime/web_tool_dispatcher.rs —
//!  kernel-floor handler for the `:web_tool` hecksagon adapter
//!  family (i569). Implements two behavior_kinds declared under
//!  hecks_conception/aggregates/framework/behavior_kinds/ :
//!  `perform_web_fetch` (HTTP GET via curl shell-out) and
//!  `perform_web_search` (DuckDuckGo HTML-lite query). Sibling
//!  kernel-floor port to claude_tool_dispatcher.rs and
//!  sms_dispatcher.rs ; both retire when i557's framework-wide
//!  kernel-hook registry replaces hard-coded handler tables. This
//!  module exposes `register_web_tool_hooks` for that registry to
//!  consume ; it deliberately does NOT touch `Runtime::dispatch`.]
//!
//! WebToolDispatcher — kernel hook for the :web_tool adapter family
//!
//! When a Tools.WebFetch / Tools.WebSearch command dispatches, the
//! runtime locates the matching `:web_tool` adapter declared in a
//! loaded hecksagon (the `framework/tools/tools.hecksagon`
//! bindings), reads its `tool` field, and calls one of the two
//! native primitives below.
//!
//! Pattern mirrors claude_tool_dispatcher : take the adapter's
//! declared fields + the dispatched command's attrs, execute,
//! return a structured result the Runtime can chain back via the
//! adapter's `result_into` target.
//!
//! ── Scope ──
//!
//! Two native primitives in v1 :
//!   * web_fetch  — curl shell-out for a plain HTTP GET. URLs are
//!                  validated for safety (no file://, no localhost,
//!                  no RFC1918 ranges) before any network call.
//!                  Captured body capped at 100 KB.
//!   * web_search — DuckDuckGo HTML-lite query (no API key). Pulls
//!                  the response HTML, extracts top-N result blocks,
//!                  formats as plain text. Capped at 100 KB.
//!
//! Network access in unit tests is GATED on the `HECKS_WEB_TOOL_LIVE`
//! env var — when unset, the live-network tests skip ; when set,
//! they hit the real internet. CI defaults to unset.
//!
//! ── Wire-in with i557 ──
//!
//! i557 is building `framework_registry.rs` — a typed registry that
//! walks `framework/adapter_families/` + `framework/behavior_kinds/`
//! at boot and dispatches kernel hooks by behavior name. Once that
//! lands, the runtime calls `register_web_tool_hooks(&mut registry)`
//! to populate the two `perform_web_*` slots. Until then this module
//! is callable but unwired — `Runtime::dispatch` is NOT modified.

use super::web_tool_requests;
use std::collections::HashMap;

/// What a :web_tool dispatch produced. Fields are populated per the
/// tool kind that ran — :web_fetch sets `output` to the response
/// body, :web_search sets `output` to the formatted result list.
/// `exit_code` carries the HTTP status (or provider status) so
/// downstream policies can branch on 2xx / 4xx / 5xx the same way
/// they branch on a shell exit.
#[derive(Debug, Clone, Default)]
pub struct WebToolResult {
    /// The tool that ran ("web_fetch" or "web_search").
    pub tool: String,
    /// True if the call succeeded — 2xx status + parseable response.
    pub ok: bool,
    /// Response body / formatted search results — kind-specific.
    pub output: String,
    /// HTTP status code (or provider status). 0 if not applicable.
    pub exit_code: i32,
    /// Human-readable error message when `ok == false`.
    pub error: Option<String>,
}

/// Dispatch a :web_tool adapter call. `tool` is the adapter's
/// declared `tool:` field (`"web_fetch"` or `"web_search"`). `attrs`
/// carries the dispatched command's attributes — `url` for fetch,
/// `query` for search.
pub fn dispatch(tool: &str, attrs: &HashMap<String, String>) -> WebToolResult {
    match tool {
        "web_fetch"  => web_tool_requests::run_web_fetch(attrs),
        "web_search" => web_tool_requests::run_web_search(attrs),
        other        => WebToolResult {
            tool: other.to_string(),
            ok: false,
            error: Some(format!("unknown web tool kind: {}", other)),
            ..Default::default()
        },
    }
}

/// Maximum bytes of captured output any web tool returns — caps
/// downstream state for large pages / wide search-result pages.
pub(super) const OUTPUT_LIMIT_BYTES: usize = 102_400; // 100 KB

pub(super) fn truncate(s: String) -> String {
    if s.len() <= OUTPUT_LIMIT_BYTES {
        s
    } else {
        let mut t = s.into_bytes();
        t.truncate(OUTPUT_LIMIT_BYTES);
        let mut out = String::from_utf8_lossy(&t).into_owned();
        out.push_str("\n\n... [truncated]");
        out
    }
}

// ── URL safety ──
//
// Reject URLs that point at the local network or local filesystem
// before any network call fires. The list mirrors RFC1918 + loopback
// + link-local. A consumer could still craft a URL pointing at an
// internal-DNS hostname that resolves to a private IP — that's a
// limitation of pre-resolution validation, called out in i569.

/// Returns Some(error_message) if the URL is unsafe, None if it's OK.
pub(super) fn validate_url(url: &str) -> Option<String> {
    let lower = url.to_lowercase();

    // Reject non-http(s) schemes outright.
    if lower.starts_with("file://") {
        return Some("rejected scheme: file://".into());
    }
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return Some(format!("rejected scheme (only http/https allowed): {}", url));
    }

    // Extract host substring : after scheme, before next '/' or ':'
    let after_scheme = if let Some(rest) = lower.strip_prefix("http://") {
        rest
    } else if let Some(rest) = lower.strip_prefix("https://") {
        rest
    } else {
        return Some("scheme parse failure".into());
    };
    let host = after_scheme
        .split(['/', ':', '?', '#'])
        .next()
        .unwrap_or("");
    if host.is_empty() {
        return Some("missing host".into());
    }

    if host == "localhost" || host == "::1" {
        return Some(format!("rejected host: {}", host));
    }

    // Numeric IPv4 check — split into octets, match against the
    // forbidden ranges. Any non-numeric host (DNS names) skips this.
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() == 4 && parts.iter().all(|p| p.parse::<u8>().is_ok()) {
        let octets: Vec<u8> = parts.iter().map(|p| p.parse::<u8>().unwrap()).collect();
        if octets[0] == 127 {
            return Some(format!("rejected loopback IP: {}", host));
        }
        if octets[0] == 10 {
            return Some(format!("rejected RFC1918 IP: {}", host));
        }
        if octets[0] == 192 && octets[1] == 168 {
            return Some(format!("rejected RFC1918 IP: {}", host));
        }
        if octets[0] == 172 && (16..=31).contains(&octets[1]) {
            return Some(format!("rejected RFC1918 IP: {}", host));
        }
        if octets[0] == 169 && octets[1] == 254 {
            return Some(format!("rejected link-local IP: {}", host));
        }
        if octets[0] == 0 {
            return Some(format!("rejected reserved IP: {}", host));
        }
    }

    None
}

pub(super) fn err(tool: &str, msg: &str, exit_code: i32) -> WebToolResult {
    WebToolResult {
        tool: tool.to_string(),
        ok: false,
        output: String::new(),
        exit_code,
        error: Some(msg.to_string()),
    }
}

// ── i557 hook registration ──
//
// i557 builds a typed kernel-hook registry (`FrameworkRegistry`) that
// walks `framework/adapter_families/` + `framework/behavior_kinds/`
// at boot. Adapter families like `:web_tool` register their kernel
// hooks into the registry by behavior name ; when a dispatch fires a
// behavior, the registry looks up the hook and runs it.
//
// This function is the registration point. It's deliberately
// loose-typed (takes a generic insert closure) because i557 is still
// in flight and the precise `FrameworkRegistry::register_hook`
// signature isn't merged to main yet. When i557 lands, the call
// site there will invoke this function with the registry's actual
// insert method ; the registry then dispatches `perform_web_fetch`
// / `perform_web_search` through the closures we register here.
//
// The two hooks share a single underlying primitive : `dispatch`
// (above), branched on `tool`. The registry passes the adapter's
// `tool` field as part of `attrs` ; we read it back and re-enter
// `dispatch`. That keeps this function trivial and lets the
// registry-side wiring stay symmetrical across families.

/// A simplified hook signature : takes the dispatched command's
/// attributes (already merged with adapter fields by the registry)
/// and returns the structured `WebToolResult`. The registry-side
/// wrapper adapts this to whatever closure shape the registry
/// expects (string→string, structured→structured, etc.).
pub type WebToolHook = fn(&HashMap<String, String>) -> WebToolResult;

/// Look up the kernel-hook closure for a given web_tool behavior
/// name. Returns `None` if the behavior isn't one we handle.
///
/// The registry calls this once per `:web_tool` behavior at boot
/// to populate its `kernel_hooks` map. Decoupled from the registry's
/// exact API so this module stays buildable before i557 merges.
pub fn lookup_hook(behavior_name: &str) -> Option<WebToolHook> {
    match behavior_name {
        "perform_web_fetch"  => Some(perform_web_fetch_hook),
        "perform_web_search" => Some(perform_web_search_hook),
        _ => None,
    }
}

fn perform_web_fetch_hook(attrs: &HashMap<String, String>) -> WebToolResult {
    web_tool_requests::run_web_fetch(attrs)
}

fn perform_web_search_hook(attrs: &HashMap<String, String>) -> WebToolResult {
    web_tool_requests::run_web_search(attrs)
}

/// The two behavior names this module owns — exposed so the
/// registry's boot path can iterate them without hardcoding the
/// list at the registry side.
pub const WEB_TOOL_BEHAVIOR_NAMES: &[&str] = &[
    "perform_web_fetch",
    "perform_web_search",
];
