//! registry_kernel_hooks — seed_kernel_hooks, the native-family hook table
//! (invoke_claude_tool i551/i556, invoke_mcp_tool i594, web_fetch /
//! web_search i569) plus the WebToolResult → KernelResult bridge. The
//! i594-temporary registration path while bluebook-driven hook discovery
//! is in design ; the registry struct + loaders stay in
//! framework_registry.rs.
//!
//! Cask extracted VERBATIM from runtime/framework_registry.rs
//! (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/registry_kernel_hooks.rs —
//!  kernel-floor hook table, relocated verbatim from framework_registry.rs
//!  blanket.]

use super::framework_registry::{FrameworkRegistry, KernelResult};
use crate::runtime::{claude_tool_dispatcher, mcp_dispatcher, web_tool_dispatcher};
use std::collections::HashMap;

/// Seed kernel hooks for the families this kernel knows natively.
///
/// Today : `invoke_claude_tool` (the i551/i556 claude_tool family)
/// and `invoke_mcp_tool` (the i593 mcp family). Future families
/// (web_tool, sms, ...) register their hooks here as their
/// kernel-side dispatchers land. The signature is the same for
/// every hook — the family declares the surface, the kernel
/// supplies the execution.
///
/// i594 — `invoke_mcp_tool` joins the table alongside
/// `invoke_claude_tool`. The :mcp adapter family (declared at
/// `aggregates/framework/adapter_families/mcp.hecksagon`) carries
/// `behavior :invoke_mcp_tool` ; without the registration below the
/// family parses cleanly but never fires. Adding the line here is
/// the i594-temporary path while bluebook-driven hook registration
/// (the i594 acceptance, an `inventory`-style discovery) is still
/// in design. Same exemption pattern as the rest of this kernel-
/// floor file.
pub(super) fn seed_kernel_hooks(registry: &mut FrameworkRegistry) {
    registry.register_hook(
        "invoke_claude_tool",
        claude_tool_dispatcher::dispatch_via_registry,
    );
    registry.register_hook(
        "invoke_mcp_tool",
        mcp_dispatcher::dispatch_via_registry,
    );
    // i569 — :web_tool family. The dispatcher's `WebToolHook` shape
    // (`fn(&attrs) -> WebToolResult`) differs from `KernelHook`
    // (`fn(&fields, &attrs) -> KernelResult`), so register named shims
    // that adapt the result type. The two `perform_web_*` behaviors
    // each get a `KernelHook` translating `WebToolResult -> KernelResult`.
    // (The live dispatch path in `Runtime::resolve_web_tool_adapters`
    // calls `web_tool_dispatcher::dispatch` directly, parallel to the
    // claude_tool / mcp arms ; this registry seeding keeps the family
    // discoverable via `lookup_hook` for the i557 generalized path.)
    registry.register_hook("perform_web_fetch", web_fetch_kernel_hook);
    registry.register_hook("perform_web_search", web_search_kernel_hook);
}

/// `KernelHook` shim for the :web_tool `perform_web_fetch` behavior.
/// Runs the dispatcher's web-fetch primitive over `command_attrs`
/// (the dispatched command's attrs) and folds its `WebToolResult` into
/// the generic `KernelResult` the registry's hook table expects.
fn web_fetch_kernel_hook(
    _adapter_fields: &HashMap<String, String>,
    command_attrs: &HashMap<String, String>,
) -> KernelResult {
    web_tool_result_to_kernel(web_tool_dispatcher::dispatch("web_fetch", command_attrs))
}

/// `KernelHook` shim for the :web_tool `perform_web_search` behavior.
fn web_search_kernel_hook(
    _adapter_fields: &HashMap<String, String>,
    command_attrs: &HashMap<String, String>,
) -> KernelResult {
    web_tool_result_to_kernel(web_tool_dispatcher::dispatch("web_search", command_attrs))
}

/// Translate the dispatcher-local `WebToolResult` into the generic
/// `KernelResult` envelope. `tool` → `kind`, body → `output`, HTTP
/// status → `exit_code`.
fn web_tool_result_to_kernel(r: web_tool_dispatcher::WebToolResult) -> KernelResult {
    KernelResult {
        kind: r.tool,
        ok: r.ok,
        output: r.output,
        exit_code: r.exit_code,
        error: r.error,
    }
}
