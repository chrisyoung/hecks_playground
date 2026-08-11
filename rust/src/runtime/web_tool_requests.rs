//! web_tool_requests — the HTTP runners of the :web_tool family :
//! run_web_fetch (curl GET, URL-safety gated, body + HTTP status) and
//! run_web_search (DuckDuckGo HTML-lite query → web_search_parse → top-N
//! results), plus the USER_AGENT contract. Dispatch doors + URL safety +
//! hook registration stay in web_tool_dispatcher.rs.
//!
//! Cask extracted VERBATIM from runtime/web_tool_dispatcher.rs
//! (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/web_tool_requests.rs — kernel-floor
//!  web-tool runners, relocated verbatim from web_tool_dispatcher.rs
//!  blanket.]

use super::web_search_parse::*;
use super::web_tool_dispatcher::{err, truncate, validate_url, WebToolResult};
use std::collections::HashMap;
use std::process::Command;

// ── :web_fetch — curl GET <url>, capture body + HTTP status ──

const USER_AGENT: &str = "hecks-web-tool/0.1 (+https://github.com/embryonaut/hecks)";

pub(super) fn run_web_fetch(attrs: &HashMap<String, String>) -> WebToolResult {
    let url = match attrs.get("url") {
        Some(u) => u.clone(),
        None => return err("web_fetch", "missing required attr: url", 400),
    };

    if let Some(reason) = validate_url(&url) {
        return err("web_fetch", &format!("url validation failed: {}", reason), 400);
    }

    // curl flags :
    //   -s    silent (no progress meter)
    //   -L    follow redirects
    //   -A    User-Agent
    //   -w    write-out HTTP status code on its own line at the end
    //   --max-time 30  cap total transfer at 30 seconds
    //   --max-filesize 1048576  cap downloaded body at 1 MB
    let output = Command::new("curl")
        .args([
            "-sL",
            "-A", USER_AGENT,
            "--max-time", "30",
            "--max-filesize", "1048576",
            "-w", "\n__HTTP_STATUS__:%{http_code}",
            &url,
        ])
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
            // The body ends with our `\n__HTTP_STATUS__:NNN` sentinel.
            // Split it back into (body, status).
            let (body, status_code) = match stdout.rfind("\n__HTTP_STATUS__:") {
                Some(idx) => {
                    let status_str = &stdout[idx + "\n__HTTP_STATUS__:".len()..];
                    let status: i32 = status_str.trim().parse().unwrap_or(0);
                    (stdout[..idx].to_string(), status)
                }
                None => (stdout, 0),
            };

            if !out.status.success() && status_code == 0 {
                let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
                return err(
                    "web_fetch",
                    &format!("curl failed: {}", stderr.trim()),
                    out.status.code().unwrap_or(-1),
                );
            }

            let ok = (200..400).contains(&status_code);
            WebToolResult {
                tool: "web_fetch".into(),
                ok,
                output: truncate(body),
                exit_code: status_code,
                error: if ok {
                    None
                } else {
                    Some(format!("HTTP {}", status_code))
                },
            }
        }
        Err(e) => err("web_fetch", &format!("spawn curl failed: {}", e), -1),
    }
}

// ── :web_search — DuckDuckGo HTML-lite query, parse top-N results ──

pub(super) fn run_web_search(attrs: &HashMap<String, String>) -> WebToolResult {
    let query = match attrs.get("query") {
        Some(q) => q.clone(),
        None => return err("web_search", "missing required attr: query", 400),
    };
    if query.trim().is_empty() {
        return err("web_search", "query is empty", 400);
    }

    let encoded = url_encode(&query);
    let url = format!("https://html.duckduckgo.com/html/?q={}", encoded);

    let output = Command::new("curl")
        .args([
            "-sL",
            "-A", USER_AGENT,
            "--max-time", "30",
            "--max-filesize", "2097152",
            "-w", "\n__HTTP_STATUS__:%{http_code}",
            &url,
        ])
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
            let (body, status_code) = match stdout.rfind("\n__HTTP_STATUS__:") {
                Some(idx) => {
                    let status_str = &stdout[idx + "\n__HTTP_STATUS__:".len()..];
                    let status: i32 = status_str.trim().parse().unwrap_or(0);
                    (stdout[..idx].to_string(), status)
                }
                None => (stdout, 0),
            };

            if !out.status.success() && status_code == 0 {
                let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
                return err(
                    "web_search",
                    &format!("curl failed: {}", stderr.trim()),
                    out.status.code().unwrap_or(-1),
                );
            }
            if !(200..400).contains(&status_code) {
                return WebToolResult {
                    tool: "web_search".into(),
                    ok: false,
                    output: truncate(body),
                    exit_code: status_code,
                    error: Some(format!("HTTP {}", status_code)),
                };
            }

            let results = parse_duckduckgo_html(&body, 10);
            if results.is_empty() {
                return WebToolResult {
                    tool: "web_search".into(),
                    ok: false,
                    output: String::new(),
                    exit_code: status_code,
                    error: Some(
                        "no results parsed (provider response shape may have changed)".into(),
                    ),
                };
            }
            let formatted = format_search_results(&results);
            WebToolResult {
                tool: "web_search".into(),
                ok: true,
                output: truncate(formatted),
                exit_code: status_code,
                error: None,
            }
        }
        Err(e) => err("web_search", &format!("spawn curl failed: {}", e), -1),
    }
}
