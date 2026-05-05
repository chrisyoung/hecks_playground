// [antibody-exempt: rust/src/server/mod.rs — kernel-floor HTTP server :
//  std::net request reader, response writer, CORS + cookie plumbing.
//  Same Trikaya-floor justification as the rest of rust/src/server/.
//  Edit for auth : read_request now returns parsed cookies ;
//  write_response_with_extra accepts a Set-Cookie / Location block.]

//! HTTP Server — JSON API for domain runtimes
//!
//! Zero-dependency HTTP server using std::net. Serves one or many
//! domains as a REST-ish API: dispatch commands, query aggregates.
//!
//! Usage:
//!   hecks-life serve pizzas.bluebook 3100
//!   hecks-life serve path/to/hecks/ 3100

pub mod routes;
pub mod multi;
pub mod html;
pub mod html_aggregate;
pub mod html_domain;
pub mod html_fixtures;
pub mod html_login;
pub mod html_help;
pub mod html_icons;
pub mod html_kpi;
pub mod html_scripts;
pub mod html_shared;
pub mod html_sidebar;
pub mod html_wizard;
pub mod html_narration;
pub mod html_policy_chain;
pub mod html_rules;
pub mod html_usage;
pub mod html_workflow;

pub use routes::route;

use crate::runtime::Runtime;
use std::cell::RefCell;
use std::io::{Write, BufRead, BufReader, Read};
use std::net::TcpListener;

/// Serve a single domain (backward-compatible entry point)
pub fn serve(rt: Runtime, port: u16) {
    let rt = RefCell::new(rt);
    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr).unwrap_or_else(|e| {
        eprintln!("Cannot bind {}: {}", addr, e);
        std::process::exit(1);
    });

    let domain_name = rt.borrow().domain.name.clone();
    eprintln!("Hecks Life — {} on http://localhost:{}", domain_name, port);

    for stream in listener.incoming().flatten() {
        handle_single(stream, &rt);
    }
}

fn handle_single(mut stream: std::net::TcpStream, rt: &RefCell<Runtime>) {
    let (method, path, body, _cookies) = match read_request(&stream) {
        Some(r) => r,
        None => return,
    };
    let (status, resp_body) = routes::route(&method, &path, &body, rt);
    write_response(&mut stream, status, &resp_body);
}

/// Read an HTTP request, return (method, path, body)
/// Parsed HTTP request : (method, path, body, cookies).
/// cookies is a flat HashMap<name, value> populated from the Cookie header.
pub fn read_request(
    stream: &std::net::TcpStream,
) -> Option<(String, String, String, std::collections::HashMap<String, String>)> {
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() { return None; }

    let parts: Vec<&str> = request_line.trim().split_whitespace().collect();
    if parts.len() < 2 { return None; }
    let method = parts[0].to_string();
    let path = parts[1].to_string();

    let mut content_length = 0usize;
    let mut cookies: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).is_err() { return None; }
        if header.trim().is_empty() { break; }
        let lower = header.to_lowercase();
        if lower.starts_with("content-length:") {
            content_length = header[15..].trim().parse().unwrap_or(0);
        } else if lower.starts_with("cookie:") {
            let raw = header[7..].trim();
            for pair in raw.split(';') {
                let pair = pair.trim();
                if let Some(eq) = pair.find('=') {
                    let k = pair[..eq].trim().to_string();
                    let v = pair[eq + 1..].trim().to_string();
                    if !k.is_empty() {
                        cookies.insert(k, v);
                    }
                }
            }
        }
    }

    let body = if content_length > 0 {
        let mut buf = vec![0u8; content_length];
        reader.read_exact(&mut buf).ok();
        String::from_utf8(buf).unwrap_or_default()
    } else {
        String::new()
    };

    Some((method, path, body, cookies))
}

/// Write an HTTP response with CORS headers. `extra` is an optional
/// header block (e.g. "Set-Cookie: hecks_session=abc; HttpOnly\r\n"
/// or "Location: /domains/BinBuddy\r\n") inserted before the blank
/// line. Pass `None` for plain responses.
pub fn write_response(stream: &mut std::net::TcpStream, status: &str, body: &str) {
    write_response_with_extra(stream, status, body, None);
}

pub fn write_response_with_extra(
    stream: &mut std::net::TcpStream,
    status: &str,
    body: &str,
    extra: Option<&str>,
) {
    let content_type = if body.starts_with("<!") || body.starts_with("<h") {
        "text/html"
    } else {
        "application/json"
    };
    let extra_block = extra.unwrap_or("");
    let resp = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
         Access-Control-Allow-Headers: Content-Type\r\n\
         {}Content-Length: {}\r\n\r\n{}",
        status, content_type, extra_block, body.len(), body
    );
    let _ = stream.write_all(resp.as_bytes());
}
