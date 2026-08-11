//! framework_registry_tests — the i557 registry suite : adapter-family /
//! behavior-kind file parsing shapes and the registry's seed + lookup
//! contract.
//!
//! Cask extracted VERBATIM from runtime/framework_registry.rs
//! (cask-runtime) ; body dedented one level out of the old inline mod.
//!
//! [antibody-exempt: rust/src/runtime/framework_registry_tests.rs —
//!  kernel-floor registry tests, relocated verbatim from
//!  framework_registry.rs blanket.]

use super::family_file_parse::*;
use super::framework_registry::*;
use std::collections::HashMap;
use std::path::Path;


#[test]
fn empty_registry_lookups_return_none() {
    let reg = FrameworkRegistry::new();
    assert!(reg.lookup_family("claude_tool").is_none());
    assert!(reg.lookup_hook("invoke_claude_tool").is_none());
    assert!(reg.families.is_empty());
    assert!(reg.behaviors.is_empty());
    assert!(reg.hooks.is_empty());
}

#[test]
fn parse_adapter_family_extracts_fields_and_behavior() {
    let source = r#"
Hecks.adapter_family "claude_tool" do
  # comment
  fields :name, :command, :tool, :result_into
  trigger_field :command
  response_field :result_into
  behavior :invoke_claude_tool
end
"#;
    let fam = parse_adapter_family_file(source).expect("parsed family");
    assert_eq!(fam.name, "claude_tool");
    assert_eq!(fam.fields, vec!["name", "command", "tool", "result_into"]);
    assert_eq!(fam.trigger_field.as_deref(), Some("command"));
    assert_eq!(fam.response_field.as_deref(), Some("result_into"));
    assert_eq!(fam.behavior.as_deref(), Some("invoke_claude_tool"));
    assert_eq!(fam.behaviors, vec!["invoke_claude_tool"]);
}

#[test]
fn parse_adapter_family_with_plural_behaviors_and_providers() {
    let source = r#"
Hecks.adapter_family "sms" do
  fields :name, :behavior, :trigger_on, :response_into
  trigger_field :trigger_on
  response_field :response_into
  behaviors :call_sms_api
  providers :twilio
end
"#;
    let fam = parse_adapter_family_file(source).expect("parsed family");
    assert_eq!(fam.name, "sms");
    assert_eq!(fam.behavior.as_deref(), Some("call_sms_api"));
    assert_eq!(fam.behaviors, vec!["call_sms_api"]);
    assert_eq!(fam.providers, vec!["twilio"]);
}

#[test]
fn parse_behavior_kind_extracts_required_and_trigger() {
    let source = r#"
Hecks.behavior_kind "invoke_claude_tool" do
  requires_field :tool
  requires_field :result_into
  trigger_attribute :description
end
"#;
    let bk = parse_behavior_kind_file(source).expect("parsed behavior");
    assert_eq!(bk.name, "invoke_claude_tool");
    assert_eq!(bk.required_fields, vec!["tool", "result_into"]);
    assert_eq!(bk.trigger_attribute.as_deref(), Some("description"));
}

#[test]
fn build_from_dir_handles_missing_directories_cleanly() {
    // Pointing at a non-existent dir should return an empty
    // registry, not panic. Kernel hooks are still seeded though.
    let reg = FrameworkRegistry::build_from_dir(Path::new(
        "/nonexistent/path/that/does/not/exist",
    ));
    assert!(reg.families.is_empty());
    assert!(reg.behaviors.is_empty());
    // Hooks are seeded regardless of disk state — they're native.
    assert!(reg.lookup_hook("invoke_claude_tool").is_some());
    // i594 — invoke_mcp_tool seeds alongside invoke_claude_tool so
    // the :mcp adapter family fires once a binding matches.
    assert!(reg.lookup_hook("invoke_mcp_tool").is_some());
}

/// End-to-end discovery test : walking the real framework dir
/// populates the claude_tool family, the invoke_claude_tool
/// behavior, AND registers the kernel hook.
///
/// Per the i557 acceptance criteria : a unit test that proves
/// `build_from_dir(framework_dir)` populates with at least the
/// claude_tool family + invoke_claude_tool behavior + the
/// registered hook.
#[test]
fn build_from_dir_walks_real_framework() {
    // Locate the framework dir by walking up from CARGO_MANIFEST_DIR.
    // The crate lives at <repo>/rust/, framework dir at
    // <repo>/hecks_conception/aggregates/framework/.
    let manifest = std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR set under cargo");
    let framework_dir = Path::new(&manifest)
        .parent()
        .expect("rust crate has parent (repo root)")
        .join("hecks_conception/aggregates/framework");

    if !framework_dir.is_dir() {
        // Some build environments (sandboxed CI, sparse checkouts)
        // may not have hecks_conception present. Don't fail the
        // build there — just skip the assertion.
        eprintln!(
            "skipping framework-discovery test ; {} not a directory",
            framework_dir.display()
        );
        return;
    }

    let reg = FrameworkRegistry::build_from_dir(&framework_dir);

    let claude_tool = reg
        .lookup_family("claude_tool")
        .expect("claude_tool family populated");
    assert_eq!(claude_tool.name, "claude_tool");
    assert_eq!(claude_tool.behavior.as_deref(), Some("invoke_claude_tool"));

    let invoke = reg
        .behaviors
        .get("invoke_claude_tool")
        .expect("invoke_claude_tool behavior populated");
    assert_eq!(invoke.name, "invoke_claude_tool");

    let hook = reg
        .lookup_hook("invoke_claude_tool")
        .expect("invoke_claude_tool hook registered");
    // Sanity : the hook is callable and returns a structured
    // result. Calling with no shell_command attr exercises the
    // error path of the underlying dispatcher.
    let result = hook(
        &{
            let mut m = HashMap::new();
            m.insert("tool".to_string(), "bash".to_string());
            m
        },
        &HashMap::new(),
    );
    assert_eq!(result.kind, "bash");
    assert!(!result.ok); // Missing shell_command → ok=false.
    assert!(result.error.is_some());
}
