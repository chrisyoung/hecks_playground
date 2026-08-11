//! web_tool_tests — the :web_tool suite : dispatch surface (fetch/search
//! arg validation, unknown tool), lookup_hook registry contract, truncate
//! cap, and the DDG parse layer (parse_duckduckgo_html, percent_decode,
//! clean_ddg_url, url_encode).
//!
//! Cask extracted VERBATIM from runtime/web_tool_dispatcher.rs
//! (cask-runtime) ; body dedented one level out of the old inline mod.
//!
//! [antibody-exempt: rust/src/runtime/web_tool_tests.rs — kernel-floor
//!  web-tool tests, relocated verbatim from web_tool_dispatcher.rs
//!  blanket.]

use super::web_search_parse::*;
use super::web_tool_dispatcher::*;
use std::collections::HashMap;


fn attrs(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

// ── URL safety ──

#[test]
fn forbidden_localhost_url_is_rejected() {
    let r = dispatch("web_fetch", &attrs(&[("url", "http://localhost:8080/x")]));
    assert!(!r.ok, "{:?}", r);
    let e = r.error.expect("error required");
    assert!(e.contains("rejected"), "got: {}", e);
}

#[test]
fn forbidden_loopback_ip_is_rejected() {
    let r = dispatch("web_fetch", &attrs(&[("url", "http://127.0.0.1/")]));
    assert!(!r.ok);
    let e = r.error.unwrap();
    assert!(e.contains("loopback") || e.contains("rejected"), "got: {}", e);
}

#[test]
fn forbidden_rfc1918_ip_is_rejected() {
    for ip in &["10.0.0.1", "192.168.1.1", "172.16.0.1"] {
        let r = dispatch("web_fetch", &attrs(&[("url", &format!("http://{}/x", ip))]));
        assert!(!r.ok, "ip {} should be rejected: {:?}", ip, r);
    }
}

#[test]
fn forbidden_file_scheme_is_rejected() {
    let r = dispatch("web_fetch", &attrs(&[("url", "file:///etc/passwd")]));
    assert!(!r.ok);
    let e = r.error.unwrap();
    assert!(e.contains("file"), "got: {}", e);
}

#[test]
fn unknown_tool_returns_error() {
    let r = dispatch("nonsense", &attrs(&[]));
    assert!(!r.ok);
    assert!(r.error.unwrap().contains("unknown web tool kind"));
}

#[test]
fn web_fetch_missing_url_errors() {
    let r = dispatch("web_fetch", &attrs(&[]));
    assert!(!r.ok);
    assert!(r.error.unwrap().contains("missing required attr: url"));
}

#[test]
fn web_search_missing_query_errors() {
    let r = dispatch("web_search", &attrs(&[]));
    assert!(!r.ok);
    assert!(r.error.unwrap().contains("missing required attr: query"));
}

#[test]
fn web_search_empty_query_errors() {
    let r = dispatch("web_search", &attrs(&[("query", "")]));
    assert!(!r.ok);
}

// ── Truncation ──

#[test]
fn truncate_caps_long_strings() {
    let huge = "x".repeat(OUTPUT_LIMIT_BYTES + 100);
    let t = truncate(huge);
    assert!(t.len() <= OUTPUT_LIMIT_BYTES + 32, "got len {}", t.len());
    assert!(t.contains("[truncated]"));
}

// ── URL helpers ──

#[test]
fn url_encode_handles_spaces_and_symbols() {
    assert_eq!(url_encode("hello world"), "hello%20world");
    assert_eq!(url_encode("a&b=c"), "a%26b%3Dc");
    assert_eq!(url_encode("simple-name.tld_v1~"), "simple-name.tld_v1~");
}

#[test]
fn percent_decode_round_trip() {
    assert_eq!(percent_decode("hello%20world"), "hello world");
    assert_eq!(percent_decode("a%26b%3Dc"), "a&b=c");
    assert_eq!(percent_decode("https%3A%2F%2Fexample.com"), "https://example.com");
}

#[test]
fn clean_ddg_url_unwraps_redirector() {
    let raw = "//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2Fpath&rut=abc";
    assert_eq!(clean_ddg_url(raw), "https://example.com/path");
}

#[test]
fn clean_ddg_url_passes_through_plain_https() {
    assert_eq!(clean_ddg_url("https://example.com/page"),
               "https://example.com/page");
}

// ── HTML parsing ──

#[test]
fn parse_duckduckgo_html_extracts_results() {
    let html = r##"<html><body>
        <div class="result">
          <a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com">Example Title</a>
          <a class="result__snippet" href="#">First snippet text</a>
        </div>
        <div class="result">
          <a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Frust-lang.org">Rust Lang</a>
          <a class="result__snippet" href="#">Second snippet text</a>
        </div>
    </body></html>"##;
    let results = parse_duckduckgo_html(html, 10);
    assert_eq!(results.len(), 2, "got: {:?}", results);
    assert_eq!(results[0].title, "Example Title");
    assert_eq!(results[0].url, "https://example.com");
    assert_eq!(results[0].snippet, "First snippet text");
    assert_eq!(results[1].title, "Rust Lang");
    assert_eq!(results[1].url, "https://rust-lang.org");
}

#[test]
fn parse_duckduckgo_html_empty_when_no_results() {
    let html = "<html><body>nothing here</body></html>";
    let results = parse_duckduckgo_html(html, 10);
    assert!(results.is_empty());
}

// ── Lookup / registration shape ──

#[test]
fn lookup_hook_resolves_both_behaviors() {
    assert!(lookup_hook("perform_web_fetch").is_some());
    assert!(lookup_hook("perform_web_search").is_some());
}

#[test]
fn lookup_hook_returns_none_for_unknown() {
    assert!(lookup_hook("perform_text_to_speech").is_none());
    assert!(lookup_hook("").is_none());
}

#[test]
fn behavior_names_lists_two_entries() {
    assert_eq!(WEB_TOOL_BEHAVIOR_NAMES.len(), 2);
    assert!(WEB_TOOL_BEHAVIOR_NAMES.contains(&"perform_web_fetch"));
    assert!(WEB_TOOL_BEHAVIOR_NAMES.contains(&"perform_web_search"));
}

// ── Live-network tests ──
//
// These hit the real internet ; gated on HECKS_WEB_TOOL_LIVE=1.
// CI defaults to unset so the suite stays hermetic.

fn live_network_enabled() -> bool {
    std::env::var("HECKS_WEB_TOOL_LIVE").ok().as_deref() == Some("1")
}

#[test]
fn live_web_fetch_against_example_com() {
    if !live_network_enabled() { return; }
    let r = dispatch("web_fetch", &attrs(&[("url", "https://example.com/")]));
    assert!(r.ok, "{:?}", r);
    assert!(r.output.contains("Example Domain"), "body: {}", r.output);
    assert_eq!(r.exit_code, 200);
}

#[test]
fn live_web_search_returns_results() {
    if !live_network_enabled() { return; }
    let r = dispatch("web_search", &attrs(&[("query", "rust lang")]));
    // Don't strictly require ok — DDG sometimes throttles, but
    // we should at least get a structured outcome.
    if r.ok {
        assert!(!r.output.is_empty());
    } else {
        assert!(r.error.is_some());
    }
}
