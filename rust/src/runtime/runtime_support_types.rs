//! runtime_support_types — the small state-adjacent types : BackendInfo
//! (one row of the i728 backend-map projection) and PendingReaction (one
//! enqueued Phase-3 reaction awaiting pump()). The Runtime struct itself
//! stays in runtime_state.rs ; old paths hold via its re-export.
//!
//! Cask extracted VERBATIM from runtime/runtime_state.rs (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/runtime_support_types.rs —
//!  kernel-floor state types, relocated verbatim from runtime_state.rs
//!  blanket.]

use super::lazy_repository::BackendKind;
use super::{CommandResult, Value};
use std::collections::HashMap;

/// One row of the backend-map projection (i728) — which backend each repository
/// resolved to, without hydrating it. `Runtime::dump_backend_map` builds the Vec ;
/// the Phase-A enforcement gate diffs it before/after a change and treats any
/// unexpected backend flip as an automatic stop.
#[derive(Debug, Clone)]
pub struct BackendInfo {
    pub repo_key: String,
    pub kind: BackendKind,
    pub heki_path: Option<String>,
}


/// Phase 3 — one enqueued reaction: a command result whose cross-
/// aggregate reactions have not yet been delivered. Held in the Runtime
/// outbox until `pump()` runs `react` for it.
#[derive(Debug, Clone)]
pub struct PendingReaction {
    pub result: CommandResult,
    pub command_name: String,
    pub attrs: HashMap<String, Value>,
}

