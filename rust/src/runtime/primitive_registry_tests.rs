//! primitive_registry_tests — the primitive-audit suite : registration,
//! signature lookup, route logging.
//!
//! Cask extracted VERBATIM from runtime/primitive_registry.rs
//! (cask-runtime) ; body dedented one level out of the old inline mod.
//!
//! [antibody-exempt: rust/src/runtime/primitive_registry_tests.rs —
//!  kernel-floor primitive tests, relocated verbatim from
//!  primitive_registry.rs blanket.]

use super::primitive_registry::*;

#[test]
fn seed_includes_process_spawn() {
    let mut reg = PrimitiveRegistry::new();
    reg.seed_builtins();
    let spec = reg.lookup("Process.Spawn").expect("Process.Spawn seeded");
    assert_eq!(spec.kind, "process");
    assert_eq!(spec.implementation, "exec_dispatcher::dispatch");
}

#[test]
fn seed_includes_every_kernel_dispatcher_kind() {
    let mut reg = PrimitiveRegistry::new();
    reg.seed_builtins();
    let kinds: Vec<&str> = reg.iter().map(|s| s.kind.as_str()).collect();
    for kind in [
        "process", "claude_tool", "mcp_tool", "web_tool",
        "compute", "llm", "shell", "sms",
    ] {
        assert!(kinds.contains(&kind), "missing kind={}", kind);
    }
}

#[test]
fn unknown_lookup_returns_none() {
    let reg = PrimitiveRegistry::new();
    assert!(reg.lookup("Unknown.Verb").is_none());
}

/// Sprint-14 macrophage : every kernel-floor `*_dispatcher.rs` in
/// `rust/src/runtime/` must have a matching seed entry. This test
/// IS the Rust-side enforcement of the `primitive_registry_declared`
/// macrophage check — when a new dispatcher lands without a seed
/// entry the test fails ; when a seed entry names a dispatcher
/// that no longer exists the test also fails. Single source of
/// truth keeps the seed list and the source tree in lockstep.
#[test]
fn every_kernel_dispatcher_has_a_seed_entry() {
    use std::fs;
    use std::path::Path;
    // CARGO_MANIFEST_DIR resolves to rust/ — the dispatcher files
    // live under src/runtime/.
    let runtime_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/runtime");
    let mut dispatcher_kinds: Vec<String> = Vec::new();
    for entry in fs::read_dir(&runtime_dir).expect("read runtime dir") {
        let path = entry.expect("dir entry").path();
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        // Match `<kind>_dispatcher.rs` ; map `exec_dispatcher.rs`
        // to the `process` kind (it is the actual implementation
        // of Primitive::Process.Spawn). Skip `command_dispatch.rs`
        // — a sibling protocol module, not a kernel-floor leaf.
        if let Some(stripped) = name.strip_suffix("_dispatcher.rs") {
            let kind = match stripped {
                "exec" => "process".to_string(),
                other  => other.to_string(),
            };
            dispatcher_kinds.push(kind);
        }
    }
    assert!(!dispatcher_kinds.is_empty(), "no dispatchers found");
    let mut reg = PrimitiveRegistry::new();
    reg.seed_builtins();
    let seeded_kinds: std::collections::HashSet<String> =
        reg.iter().map(|s| s.kind.clone()).collect();
    for kind in &dispatcher_kinds {
        assert!(
            seeded_kinds.contains(kind),
            "kernel-floor dispatcher kind={} has no Storehouse::Primitive seed entry — add one to PrimitiveRegistry::seed_builtins (and a Storehouse::Primitive.Conceive declaration)",
            kind,
        );
    }
}
