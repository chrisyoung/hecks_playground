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
use super::{read_request, write_response};
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
    let (method, path, body) = match read_request(&stream) {
        Some(r) => r,
        None => return,
    };

    let seg: Vec<&str> = path.trim_matches('/').split('/').collect();
    let (status, resp_body) = route_multi(&method, &seg, &body, runtimes);
    write_response(&mut stream, status, &resp_body);
}

fn route_multi(
    method: &str, seg: &[&str], body: &str,
    runtimes: &HashMap<String, RefCell<Runtime>>,
) -> (&'static str, String) {
    match (method, seg) {
        ("OPTIONS", _) => ("204 No Content", String::new()),

        ("GET", [""]) | ("GET", []) => {
            ("200 OK", html::generate_index(runtimes))
        }

        ("GET", ["domains"]) => {
            let list: Vec<String> = domain_list(runtimes);
            let items: Vec<String> = list.iter()
                .map(|n| format!(r#""{}""#, n)).collect();
            ("200 OK", format!(
                r#"{{"count":{},"domains":[{}]}}"#, items.len(), items.join(",")
            ))
        }

        ("GET", ["domains", name]) => {
            match runtimes.get(*name) {
                Some(rt) => ("200 OK", html_domain::generate_domain_page(name, rt, runtimes)),
                None => ("404 Not Found", format!(
                    r#"{{"error":"domain not found","name":"{}"}}"#, name
                )),
            }
        }

        // Per-aggregate focused page : `/domains/<Name>/aggregates/<AggName>`
        // — center panel renders only the named aggregate's bluebook
        // (header, attributes, value_objects, references, lifecycle,
        // commands as runnable forms, queries). Click an aggregate in
        // the left nav and the page filters to just that one aggregate.
        ("GET", ["domains", name, "aggregates", agg]) => {
            match runtimes.get(*name) {
                Some(rt) => ("200 OK", html_aggregate::generate_aggregate_page(name, agg, rt, runtimes)),
                None => ("404 Not Found", format!(
                    r#"{{"error":"domain not found","name":"{}"}}"#, name
                )),
            }
        }

        ("GET", ["domains", name, rest @ ..]) |
        ("POST", ["domains", name, rest @ ..]) => {
            match runtimes.get(*name) {
                Some(rt) => {
                    let sub = format!("/{}", rest.join("/"));
                    routes::route(method, &sub, body, rt)
                }
                None => ("404 Not Found", format!(
                    r#"{{"error":"domain not found","name":"{}"}}"#, name
                )),
            }
        }

        // Fall through to single-domain style for health
        ("GET", ["health"]) => ("200 OK", r#"{"status":"ok"}"#.into()),

        _ => ("404 Not Found", r#"{"error":"not found"}"#.into()),
    }
}

fn domain_list(runtimes: &HashMap<String, RefCell<Runtime>>) -> Vec<String> {
    let mut names: Vec<String> = runtimes.keys().cloned().collect();
    names.sort();
    names
}

