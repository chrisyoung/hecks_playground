//! FrameworkRegistry — typed runtime registry for Phase-2 framework
//!
//! [antibody-exempt: rust/src/runtime/framework_registry.rs — kernel-floor
//!  runtime registry (i557 Phase-2). Builds typed adapter-family / behavior-
//!  kind registries from bluebook definitions and routes dispatch through
//!  them ; a registry OF kernel hooks cannot itself be a bluebook.]
//!
//! i557 names the Phase-2 framework runtime: at boot, walk
//! `framework/adapter_families/` + `framework/behavior_kinds/`, build
//! typed registries from the bluebook definitions, and route dispatch
//! through them. Today the parser already stamps `framework_kind` on
//! Hecksagon IR but the runtime ignores it — this module closes that
//! gap. Part 1 (this card) lands the registry surface + boot wiring +
//! kernel hook registration. Part 2 retires the hardcoded `:claude_tool`
//! shortcut from `Runtime::dispatch`.
//!
//! Contract:
//!   - `AdapterFamily` describes a category of adapter declared under
//!     `framework/adapter_families/<name>.hecksagon`. Fields, trigger
//!     attribute, response attribute, and the behavior it implements
//!     come from the hecksagon body ; `providers` lists concrete impls.
//!   - `BehaviorKind` describes a verb the kernel can run on behalf of
//!     a family (`framework/behavior_kinds/<name>.hecksagon`). It names
//!     the fields the behavior requires and which command attribute
//!     triggers it.
//!   - `KernelHook` is the native function the runtime calls when a
//!     family's behavior fires. The registry stores hooks by behavior
//!     name so a dispatch knows which closure to run. Today the only
//!     seeded hook is `invoke_claude_tool` ; sms / web_tool /
//!     future behaviors slot in here without runtime edits.
//!   - `FrameworkRegistry` is the lookup table: family + behavior maps
//!     keyed by name plus the hook map. `build_from_dir` walks a root
//!     and populates the maps ; `lookup_family` resolves an IR
//!     adapter's `kind` to the family that owns it, and `lookup_hook`
//!     resolves a behavior name to its native handler.
//!
//! Parser gap (i557 follow-up) : the Hecksagon parser captures
//! `framework_kind = "adapter_family"` / `"behavior_kind"` but NOT the
//! inner declarations (`fields`, `behavior`, `requires_field`, etc.).
//! Per i557 option #2, this module re-parses the framework files at
//! boot via a small dedicated reader (`parse_adapter_family_file` /
//! `parse_behavior_kind_file`). The structurally honest path is
//! extending the IR ; that work is filed as a follow-up so this card
//! ships the registry surface without parser ripple.
//!
//! Usage:
//!   let reg = FrameworkRegistry::build_from_dir(framework_dir);
//!   if let Some(family) = reg.lookup_family("claude_tool") { ... }
//!   if let Some(hook)   = reg.lookup_hook("invoke_claude_tool") {
//!       let result = hook(&adapter_fields, &command_attrs);
//!   }
//!
//! [antibody-exempt: rust/src/runtime/framework_registry.rs —
//!  kernel-floor module per i557. The framework registry is what
//!  *enables* bluebook-driven dispatch ; it cannot itself be bluebook-
//!  described until the very mechanism it provides is live. Retire
//!  this marker when `framework/adapter_families/` +
//!  `framework/behavior_kinds/` generate this module via the meta-
//!  shape compiler (i557 follow-up).]

use std::collections::HashMap;
use std::path::Path;

use super::family_file_parse::{parse_adapter_family_file, parse_behavior_kind_file};
use crate::runtime::claude_tool_dispatcher;
use crate::runtime::mcp_dispatcher;
use crate::runtime::web_tool_dispatcher;

/// A category of adapter declared under
/// `framework/adapter_families/<name>.hecksagon`.
///
/// Example: a `claude_tool` family has fields like `name`, `command`,
/// `tool`, `result_into`, with `command` as the trigger field,
/// `result_into` as the response field, and `invoke_claude_tool` as the
/// behavior. Providers list the concrete adapter names that implement
/// the family.
#[derive(Debug, Clone, Default)]
pub struct AdapterFamily {
    pub name: String,
    pub fields: Vec<String>,
    pub trigger_field: Option<String>,
    pub response_field: Option<String>,
    /// Primary behavior — most families have one ; multi-behavior
    /// families take the first declared one as primary, with the full
    /// list in `behaviors`.
    pub behavior: Option<String>,
    /// All declared behaviors. Single-behavior families have one entry
    /// ; multi-behavior families (e.g. a future `:web_tool` with
    /// perform_web_fetch + perform_web_search) carry several.
    pub behaviors: Vec<String>,
    pub providers: Vec<String>,
}

/// A verb the kernel can run on behalf of a family.
///
/// Declared under `framework/behavior_kinds/<name>.hecksagon`. Names
/// the fields required to invoke the behavior and which command
/// attribute triggers it at dispatch time.
#[derive(Debug, Clone, Default)]
pub struct BehaviorKind {
    pub name: String,
    pub required_fields: Vec<String>,
    pub trigger_attribute: Option<String>,
}

/// Result envelope returned by every kernel hook.
///
/// Generic enough to carry the existing `ClaudeToolResult` shape
/// (tool/ok/output/exit_code) and future families' results (e.g.
/// `:sms` sends, `:web_tool` HTTP responses).
/// The dispatcher folds these fields into the follow-on cascade attrs.
#[derive(Debug, Clone, Default)]
pub struct KernelResult {
    /// Family-specific identifier of what ran ("bash", "edit",
    /// "send_message", "perform_web_fetch", etc.). Maps to the
    /// adapter's :tool / :operation field per family convention.
    pub kind: String,
    /// True if the hook succeeded.
    pub ok: bool,
    /// stdout / response body / file contents / etc.
    pub output: String,
    /// Shell exit code (or HTTP status) when applicable ; 0 for non-
    /// shell hooks.
    pub exit_code: i32,
    /// Human-readable error when `ok == false`.
    pub error: Option<String>,
}

/// Native handler bound to a behavior name.
///
/// The runtime calls this when a family's behavior fires. The hook
/// receives the adapter's declared field values + the dispatched
/// command's attrs (both as String maps for kernel-floor simplicity)
/// and returns a structured `KernelResult` the dispatcher routes back
/// through the family's `response_field`.
///
/// Function pointer (not boxed closure) so the registry stays cheaply
/// Clone-friendly and the indirection is one jump.
pub type KernelHook =
    fn(adapter_fields: &HashMap<String, String>, command_attrs: &HashMap<String, String>) -> KernelResult;

/// Typed registry of framework families, behaviors, and their hooks.
///
/// One instance per running Runtime ; populated at boot by
/// `build_from_dir(framework_dir)` which walks the framework hecksagons
/// + seeds the known kernel hooks. The dispatcher consults
/// `lookup_family` when an IR adapter carries a `kind` matching a
/// registered family, then `lookup_hook` to find the native handler.
#[derive(Default, Clone)]
pub struct FrameworkRegistry {
    pub families: HashMap<String, AdapterFamily>,
    pub behaviors: HashMap<String, BehaviorKind>,
    pub hooks: HashMap<String, KernelHook>,
}

impl FrameworkRegistry {
    /// Empty registry — no families, no behaviors, no hooks.
    pub fn new() -> Self {
        FrameworkRegistry {
            families: HashMap::new(),
            behaviors: HashMap::new(),
            hooks: HashMap::new(),
        }
    }

    /// Walk `<framework_dir>/adapter_families/*.hecksagon` and
    /// `<framework_dir>/behavior_kinds/*.hecksagon`, populate the
    /// family + behavior maps, then seed known kernel hooks.
    ///
    /// Per i557 option #2, this uses a small dedicated reader rather
    /// than extending the main Hecksagon parser. Files that fail to
    /// read or parse are silently skipped — the registry stays usable
    /// even if one family's file is missing or malformed.
    pub fn build_from_dir(framework_dir: &Path) -> Self {
        let mut reg = Self::new();

        let families_dir = framework_dir.join("adapter_families");
        if let Ok(entries) = std::fs::read_dir(&families_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "hecksagon").unwrap_or(false) {
                    if let Ok(source) = std::fs::read_to_string(&path) {
                        if let Some(fam) = parse_adapter_family_file(&source) {
                            reg.families.insert(fam.name.clone(), fam);
                        }
                    }
                }
            }
        }

        let behaviors_dir = framework_dir.join("behavior_kinds");
        if let Ok(entries) = std::fs::read_dir(&behaviors_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "hecksagon").unwrap_or(false) {
                    if let Ok(source) = std::fs::read_to_string(&path) {
                        if let Some(bk) = parse_behavior_kind_file(&source) {
                            reg.behaviors.insert(bk.name.clone(), bk);
                        }
                    }
                }
            }
        }

        seed_kernel_hooks(&mut reg);
        reg
    }

    /// Insert a native handler bound to a behavior name.
    ///
    /// Behaviors are hecksagon-declared but their implementations are
    /// native code. This is the seam between the two — the dispatcher
    /// looks up the hook by behavior name when a family fires.
    pub fn register_hook(&mut self, behavior_name: &str, hook: KernelHook) {
        self.hooks.insert(behavior_name.to_string(), hook);
    }

    /// Resolve a behavior name to its native handler.
    pub fn lookup_hook(&self, behavior_name: &str) -> Option<KernelHook> {
        self.hooks.get(behavior_name).copied()
    }

    /// Resolve an IR adapter `kind` to the family that owns it.
    pub fn lookup_family(&self, family_name: &str) -> Option<&AdapterFamily> {
        self.families.get(family_name)
    }
}

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
fn seed_kernel_hooks(registry: &mut FrameworkRegistry) {
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
