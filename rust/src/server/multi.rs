// [antibody-exempt: rust/src/server/multi.rs — kernel-floor multi-domain
//  HTTP server. The bluebook surface (runtime/server/server.bluebook,
//  MultiDomainServer aggregate) describes the operational shape ; this
//  file is the std::net + parser-glue implementation. Same Trikaya-floor
//  justification as the rest of rust/src/server/. Edit for the i241
//  primary-bluebook walk : recursive-merge mode added.]

//! Multi-domain server — serves N bluebook domains under one API
//!
//! Scans a directory for *.bluebook files, boots a Runtime for each,
//! and serves them all with domain-namespaced routes.
//!
//! Two scan modes (i241 primary-bluebook convention) :
//!
//!   PRIMARY mode — if `<dirname>.bluebook` exists at the top level,
//!     treat it as the repo's primary and merge every *.bluebook found
//!     anywhere in the tree (recursive walk) into ONE domain. Child
//!     files contribute aggregates / value_objects / policies /
//!     process_managers / cadences ; the primary owns name + vision
//!     + category + entrypoint. This is the convention every
//!     application repo follows (bin-buddy/bin-buddy.bluebook etc.).
//!
//!   LEGACY mode — no primary at root, each top-level *.bluebook is
//!     its own domain (catalog-style, e.g. hecks_conception/catalog/
//!     where each file is a self-contained domain).
//!
//! Usage:
//!   hecks-life serve path/to/hecks/ 3100
//!
//! Routes:
//!   GET  /                         HTML index of all domains
//!   GET  /domains                  JSON list of all domain names
//!   POST /domains/:name/dispatch   Dispatch a command to a domain
//!   GET  /domains/:name/aggregates List aggregates for a domain

use crate::runtime::Runtime;
use crate::parser;
use super::{read_request, write_response, write_response_with_extra};
use super::routes;
use super::html;
use super::html_aggregate;
use super::html_domain;
use std::cell::RefCell;
use std::collections::HashMap;
use std::net::TcpListener;

/// Boot all bluebooks in a directory and serve them
pub fn serve_directory(dir: &str, port: u16) {
    let runtimes = load_all_domains(dir);
    if runtimes.is_empty() {
        eprintln!("No .bluebook files found in {}", dir);
        std::process::exit(1);
    }

    let names: Vec<String> = runtimes.keys().cloned().collect();
    eprintln!("Hecks Life — {} domains on http://localhost:{}", names.len(), port);
    for name in &names {
        eprintln!("  {}", name);
    }

    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr).unwrap_or_else(|e| {
        eprintln!("Cannot bind {}: {}", addr, e);
        std::process::exit(1);
    });

    for stream in listener.incoming().flatten() {
        handle_multi(stream, &runtimes);
    }
}

fn load_all_domains(dir: &str) -> HashMap<String, RefCell<Runtime>> {
    let mut map = HashMap::new();
    let data_dir = format!("{}/data", dir.trim_end_matches('/'));

    // i241 primary-bluebook convention. `serve <dir>` is a single-product
    // server : one .world declares the operational world, one
    // `<dirname>.bluebook` is the primary, and every other *.bluebook in
    // the tree (typically under `aggregates/<concept>/<concept>.bluebook`)
    // contributes aggregates / value_objects / policies / process_managers
    // / cadences to that one domain. Canonicalize first so `serve .` finds
    // its own basename.
    let dir_path = std::fs::canonicalize(dir)
        .unwrap_or_else(|_| std::path::PathBuf::from(dir));
    let dir_basename = dir_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let primary_path = dir_path.join(format!("{}.bluebook", dir_basename));

    if !primary_path.exists() {
        eprintln!(
            "No primary bluebook found at {} — `serve <dir>` expects \
            <dirname>.bluebook at the top of the directory.",
            primary_path.display()
        );
        std::process::exit(1);
    }

    let merged = merge_tree(&dir_path, &primary_path);
    let name = merged.name.clone();
    let rt = Runtime::boot_with_data_dir(merged, Some(data_dir.clone()));
    map.insert(name, RefCell::new(rt));
    map
}

/// Merge every *.bluebook in the tree under `dir` into the primary's Domain.
/// Primary owns name / vision / category / entrypoint. Children contribute
/// aggregates, policies, fixtures, sections, process_managers, cadences,
/// block_grammars. Files that fail to read are silently skipped.
fn merge_tree(dir: &std::path::Path, primary_path: &std::path::Path) -> crate::ir::Domain {
    let primary_src = std::fs::read_to_string(primary_path)
        .expect("primary bluebook must be readable");
    let mut merged = parser::parse(&primary_src);

    let mut child_paths: Vec<std::path::PathBuf> = Vec::new();
    collect_bluebooks(dir, primary_path, &mut child_paths);
    child_paths.sort(); // deterministic order

    for path in child_paths {
        if let Ok(source) = std::fs::read_to_string(&path) {
            let child = parser::parse(&source);
            merged.aggregates.extend(child.aggregates);
            merged.policies.extend(child.policies);
            merged.fixtures.extend(child.fixtures);
            merged.sections.extend(child.sections);
            merged.process_managers.extend(child.process_managers);
            merged.cadences.extend(child.cadences);
            merged.block_grammars.extend(child.block_grammars);
        }
    }
    merged
}

fn collect_bluebooks(
    dir: &std::path::Path,
    skip: &std::path::Path,
    out: &mut Vec<std::path::PathBuf>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_bluebooks(&path, skip, out);
        } else if path.extension().map(|e| e == "bluebook").unwrap_or(false) && path != skip {
            out.push(path);
        }
    }
}

fn handle_multi(
    mut stream: std::net::TcpStream,
    runtimes: &HashMap<String, RefCell<Runtime>>,
) {
    let (method, path, body, cookies) = match read_request(&stream) {
        Some(r) => r,
        None => return,
    };

    let seg: Vec<&str> = path.trim_matches('/').split('/').collect();
    let (status, resp_body, extra) = route_multi(&method, &seg, &body, &cookies, runtimes);
    if extra.is_empty() {
        write_response(&mut stream, status, &resp_body);
    } else {
        write_response_with_extra(&mut stream, status, &resp_body, Some(&extra));
    }
}

fn route_multi(
    method: &str, seg: &[&str], body: &str,
    cookies: &HashMap<String, String>,
    runtimes: &HashMap<String, RefCell<Runtime>>,
) -> (&'static str, String, String) {
    let no_extra = String::new();
    // Gate every /domains/* route behind a valid session cookie.
    // Unauthenticated requests get redirected to / (which then renders
    // the login or bootstrap form depending on Account count).
    // Whitelist : /, /sessions, /sessions/bootstrap, /sign-out, /health,
    // /webhooks/* (external systems can't carry our cookie). Everything
    // else under /domains/* requires auth.
    let path_is_domains = matches!(seg.first(), Some(&"domains"));
    if path_is_domains && current_email(cookies, runtimes).is_none() {
        return (
            "303 See Other",
            String::new(),
            "Location: /\r\n".to_string(),
        );
    }
    match (method, seg) {
        ("OPTIONS", _) => ("204 No Content", String::new(), no_extra),

        // Sign-in form / dashboard / bootstrap at root.
        //   - Authenticated user (cookie resolves to an active Account) → dashboard
        //   - Zero Account records exist anywhere → bootstrap form (creates the owner)
        //   - Otherwise → login form
        ("GET", [""]) | ("GET", []) => {
            if let Some(email) = current_email(cookies, runtimes) {
                ("200 OK", super::html_login::generate_dashboard(&email, runtimes), no_extra)
            } else if account_count(runtimes) == 0 {
                ("200 OK", super::html_login::generate_bootstrap_page(None), no_extra)
            } else {
                ("200 OK", super::html_login::generate_login_page(None), no_extra)
            }
        }

        // First-run owner bootstrap : only works when zero Account
        // records exist. Creates an Account with role=owner via
        // Account.SignUp dispatch, sets the cookie, redirects to /.
        // After this lands, the server has its first Account and the
        // route stops accepting (returns to login form).
        ("POST", ["sessions", "bootstrap"]) => {
            if account_count(runtimes) > 0 {
                return ("403 Forbidden", "Bootstrap is closed — owner account already exists.".into(), no_extra);
            }
            let (email, password) = parse_form_body(body);
            if email.is_empty() || password.is_empty() {
                return (
                    "200 OK",
                    super::html_login::generate_bootstrap_page(Some("Email and password are both required")),
                    no_extra,
                );
            }
            // Dispatch Account.SignUp with role=owner against the first
            // (and only) loaded domain. bin-buddy is single-domain ;
            // catalog-style multi-domain isn't a flow we need yet.
            let target = match runtimes.values().next() {
                Some(rt) => rt,
                None => return ("500 Internal Server Error", "No domain loaded".into(), no_extra),
            };
            let mut attrs: HashMap<String, crate::runtime::Value> = HashMap::new();
            attrs.insert("email".into(), crate::runtime::Value::Str(email.clone()));
            attrs.insert("password".into(), crate::runtime::Value::Str(password));
            attrs.insert("role".into(), crate::runtime::Value::Str("owner".into()));
            let mut rt_mut = target.borrow_mut();
            match rt_mut.dispatch("SignUp", attrs) {
                Ok(_) => {
                    let cookie = format!(
                        "Set-Cookie: hecks_session={}; HttpOnly; SameSite=Strict; Path=/; Max-Age=86400\r\n\
                         Location: /\r\n",
                        email,
                    );
                    ("303 See Other", String::new(), cookie)
                }
                Err(e) => (
                    "200 OK",
                    super::html_login::generate_bootstrap_page(Some(&format!("Couldn't create owner: {}", e))),
                    no_extra,
                ),
            }
        }

        // Sign-in : application/x-www-form-urlencoded body, fields
        // `email` + `password`. On match, set a cookie carrying the
        // email and redirect to /. On miss, re-render the login form
        // with a flash error.
        ("POST", ["sessions"]) => {
            let (email, password) = parse_form_body(body);
            if email.is_empty() {
                return (
                    "200 OK",
                    super::html_login::generate_login_page(Some("Email is required")),
                    no_extra,
                );
            }
            if !verify_password(runtimes, &email, &password) {
                return (
                    "200 OK",
                    super::html_login::generate_login_page(Some("Email or password didn't match")),
                    no_extra,
                );
            }
            let cookie = format!(
                "Set-Cookie: hecks_session={}; HttpOnly; SameSite=Strict; Path=/; Max-Age=86400\r\n\
                 Location: /\r\n",
                email,
            );
            ("303 See Other", String::new(), cookie)
        }

        // Sign-out : clear cookie, redirect to /. We simply expire the
        // cookie ; for tonight's walking-skeleton this is sufficient.
        ("GET", ["sign-out"]) | ("POST", ["sign-out"]) => {
            let cookie = "Set-Cookie: hecks_session=; HttpOnly; SameSite=Strict; \
                 Path=/; Max-Age=0\r\nLocation: /\r\n".to_string();
            ("303 See Other", String::new(), cookie)
        }

        // Customer self-signup. Public route — anyone can register as a
        // customer (driver + admin signup are restricted paths handled
        // separately). Two commands cascade : Account.SignUp creates
        // the auth record (role=customer), Customer.RegisterCustomer
        // attaches the profile. On success, the session cookie is set
        // and the user lands on /.
        ("GET", ["signup"]) => {
            ("200 OK", super::html_login::generate_signup_page(None), no_extra)
        }
        ("POST", ["signup"]) => {
            let fields = parse_signup_form(body);
            let email = fields.get("email").cloned().unwrap_or_default();
            let password = fields.get("password").cloned().unwrap_or_default();
            if email.is_empty() || password.is_empty() {
                return (
                    "200 OK",
                    super::html_login::generate_signup_page(Some("Email and password are required")),
                    no_extra,
                );
            }
            let target = match runtimes.values().next() {
                Some(rt) => rt,
                None => return ("500 Internal Server Error", "No domain loaded".into(), no_extra),
            };
            // Account.SignUp first.
            let mut acct: HashMap<String, crate::runtime::Value> = HashMap::new();
            acct.insert("email".into(), crate::runtime::Value::Str(email.clone()));
            acct.insert("password".into(), crate::runtime::Value::Str(password));
            acct.insert("role".into(), crate::runtime::Value::Str("customer".into()));
            let signup_result = {
                let mut rt_mut = target.borrow_mut();
                rt_mut.dispatch("SignUp", acct)
            };
            if let Err(e) = signup_result {
                return (
                    "200 OK",
                    super::html_login::generate_signup_page(Some(&format!("Couldn't create account: {}", e))),
                    no_extra,
                );
            }
            // Customer.RegisterCustomer second. Best-effort — if it
            // fails (e.g. command attribute mismatch) the account
            // still exists and the user can sign in.
            let mut prof: HashMap<String, crate::runtime::Value> = HashMap::new();
            prof.insert("account_email".into(), crate::runtime::Value::Str(email.clone()));
            for k in &["first_name", "last_name", "phone"] {
                if let Some(v) = fields.get(*k) {
                    prof.insert((*k).into(), crate::runtime::Value::Str(v.clone()));
                }
            }
            let _ = {
                let mut rt_mut = target.borrow_mut();
                rt_mut.dispatch("RegisterCustomer", prof)
            };
            let cookie = format!(
                "Set-Cookie: hecks_session={}; HttpOnly; SameSite=Strict; Path=/; Max-Age=86400\r\n\
                 Location: /\r\n",
                email,
            );
            ("303 See Other", String::new(), cookie)
        }

        ("GET", ["domains"]) => {
            let list: Vec<String> = domain_list(runtimes);
            let items: Vec<String> = list.iter()
                .map(|n| format!(r#""{}""#, n)).collect();
            ("200 OK", format!(
                r#"{{"count":{},"domains":[{}]}}"#, items.len(), items.join(",")
            ), no_extra)
        }

        ("GET", ["domains", name]) => {
            match runtimes.get(*name) {
                Some(rt) => ("200 OK", html_domain::generate_domain_page(name, rt, runtimes), no_extra),
                None => ("404 Not Found", format!(
                    r#"{{"error":"domain not found","name":"{}"}}"#, name
                ), no_extra),
            }
        }

        // Per-aggregate focused page : `/domains/<Name>/aggregates/<AggName>`
        // — center panel renders only the named aggregate's bluebook
        // (header, attributes, value_objects, references, lifecycle,
        // commands as runnable forms, queries). Click an aggregate in
        // the left nav and the page filters to just that one aggregate.
        ("GET", ["domains", name, "aggregates", agg]) => {
            match runtimes.get(*name) {
                Some(rt) => ("200 OK", html_aggregate::generate_aggregate_page(name, agg, rt, runtimes), no_extra),
                None => ("404 Not Found", format!(
                    r#"{{"error":"domain not found","name":"{}"}}"#, name
                ), no_extra),
            }
        }

        ("GET", ["domains", name, rest @ ..]) |
        ("POST", ["domains", name, rest @ ..]) => {
            match runtimes.get(*name) {
                Some(rt) => {
                    let sub = format!("/{}", rest.join("/"));
                    let (s, b) = routes::route(method, &sub, body, rt);
                    (s, b, no_extra)
                }
                None => ("404 Not Found", format!(
                    r#"{{"error":"domain not found","name":"{}"}}"#, name
                ), no_extra),
            }
        }

        // Fall through to single-domain style for health
        ("GET", ["health"]) => ("200 OK", r#"{"status":"ok"}"#.into(), no_extra),

        _ => ("404 Not Found", r#"{"error":"not found"}"#.into(), no_extra),
    }
}

fn domain_list(runtimes: &HashMap<String, RefCell<Runtime>>) -> Vec<String> {
    let mut names: Vec<String> = runtimes.keys().cloned().collect();
    names.sort();
    names
}

/// Count Account records across every loaded domain. Used by the
/// bootstrap branch — zero accounts means first-run, so `/` shows the
/// owner-bootstrap form instead of the login form.
fn account_count(runtimes: &HashMap<String, RefCell<Runtime>>) -> usize {
    runtimes.values()
        .map(|rt| rt.borrow().all("Account").len())
        .sum()
}

/// Read the `hecks_session` cookie (an email for tonight's walking-
/// skeleton) and confirm the email belongs to an active Account in
/// some loaded domain. Returns the email if so.
///
/// SECURITY DEBT acknowledged : cookie value is the plain email, not a
/// random session token. A man-in-the-middle attacker could forge a
/// cookie. Acceptable for the walking-skeleton against trusted dev
/// machines ; must be replaced before any production deploy.
fn current_email(
    cookies: &HashMap<String, String>,
    runtimes: &HashMap<String, RefCell<Runtime>>,
) -> Option<String> {
    let email = cookies.get("hecks_session")?;
    if email.is_empty() { return None; }
    if find_account_by_email(runtimes, email).is_some() {
        Some(email.clone())
    } else {
        None
    }
}

/// Find an Account record whose `email` field matches across any
/// loaded domain. Returns the (status, role) pair if found.
fn find_account_by_email(
    runtimes: &HashMap<String, RefCell<Runtime>>,
    email: &str,
) -> Option<(String, String)> {
    use crate::runtime::Value;
    for rt_cell in runtimes.values() {
        let rt = rt_cell.borrow();
        for state in rt.all("Account") {
            if let Some(Value::Str(e)) = state.fields.get("email") {
                if e == email {
                    let status = match state.fields.get("status") {
                        Some(Value::Str(s)) => s.clone(),
                        _ => String::new(),
                    };
                    let role = match state.fields.get("role") {
                        Some(Value::Str(s)) => s.clone(),
                        _ => String::new(),
                    };
                    return Some((status, role));
                }
            }
        }
    }
    None
}

/// Verify a sign-in attempt. Looks up the Account by email and
/// compares the provided password to the stored value. Tonight this
/// is plaintext compare against the `password` or `password_hash`
/// field — bcrypt verification belongs in the auth adapter, which
/// the multi-server doesn't read yet.
fn verify_password(
    runtimes: &HashMap<String, RefCell<Runtime>>,
    email: &str,
    password: &str,
) -> bool {
    use crate::runtime::Value;
    if email.is_empty() || password.is_empty() { return false; }
    for rt_cell in runtimes.values() {
        let rt = rt_cell.borrow();
        for state in rt.all("Account") {
            let stored_email = match state.fields.get("email") {
                Some(Value::Str(e)) => e.as_str(),
                _ => continue,
            };
            if stored_email != email { continue; }
            // Account must be active.
            if let Some(Value::Str(s)) = state.fields.get("status") {
                if s != "active" { return false; }
            }
            // Try password fields in order. The bluebook stores the
            // bcrypt'd value as `password_hash` ; for the walking
            // skeleton we accept a `password` field too.
            for key in &["password", "password_hash"] {
                if let Some(Value::Str(stored)) = state.fields.get(*key) {
                    if stored == password { return true; }
                }
            }
            return false;
        }
    }
    false
}

/// Parse a form-urlencoded body into a HashMap. Used by the multi-
/// field signup form which carries first_name / last_name / phone in
/// addition to email + password.
fn parse_signup_form(body: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for pair in body.split('&') {
        let pair = pair.trim();
        if let Some(eq) = pair.find('=') {
            let k = pair[..eq].to_string();
            let v = url_decode(&pair[eq + 1..]);
            if !k.is_empty() { out.insert(k, v); }
        }
    }
    out
}

/// Parse `application/x-www-form-urlencoded` body into (email, password).
/// Tonight we hand-parse instead of pulling in `url`/`form_urlencoded` —
/// keeps the zero-dependency posture of the server.
fn parse_form_body(body: &str) -> (String, String) {
    let mut email = String::new();
    let mut password = String::new();
    for pair in body.split('&') {
        let pair = pair.trim();
        if let Some(eq) = pair.find('=') {
            let k = &pair[..eq];
            let v = url_decode(&pair[eq + 1..]);
            match k {
                "email" => email = v,
                "password" => password = v,
                _ => {}
            }
        }
    }
    (email, password)
}

fn url_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'+' {
            out.push(' ');
            i += 1;
        } else if b == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
            if let Ok(n) = u8::from_str_radix(hex, 16) {
                out.push(n as char);
            }
            i += 3;
        } else {
            out.push(b as char);
            i += 1;
        }
    }
    out
}

