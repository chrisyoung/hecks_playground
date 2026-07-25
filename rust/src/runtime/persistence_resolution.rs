//! Runtime persistence resolution (i728) — the backend-map projection read
//! + the dormant is-wired check.
//!
//! Inherent `impl Runtime` methods in a child module : child modules see the
//! parent's private fields + helpers, so these reach `self.repositories` /
//! `self.hecksagons` / `repo_key` directly.
//!
//! [antibody-exempt: rust/src/runtime/persistence_resolution.rs — hand-written
//!  runtime kernel-floor. Was a codegen/runtime_shape artifact ; the shape was
//!  retired 2026-06-27 — a .bluebook that only re-emitted imperative Rust
//!  captures no domain, so the runtime kernel is hand-maintained Rust like
//!  mod.rs.]

use super::*;

// apply_wired_adapters is the only #[cfg(not(wasm32))] method here and the only
// user of HashMap ; gate the import to match so the wasm build carries no
// unused-import warning. Everything else resolves through the `super::*` glob
// (BackendInfo, LazyRepository, repo_key, PersistenceSpec, the registry fns).

impl Runtime {
    /// Snapshot every repository's resolved backend (kind + heki path) WITHOUT
    /// hydrating any of them. The i728 Phase-A enforcement gate diffs this
    /// before/after a change ; any unexpected backend flip is an automatic stop.
    /// Sorted by repo_key so the diff is deterministic.
    pub fn dump_backend_map(&self) -> Vec<BackendInfo> {
        let mut rows: Vec<BackendInfo> = self
            .repositories
            .iter()
            .map(|(key, repo)| BackendInfo {
                repo_key: key.clone(),
                kind: repo.backend_kind(),
                heki_path: repo.heki_path(),
            })
            .collect();
        rows.sort_by(|a, b| a.repo_key.cmp(&b.repo_key));
        rows
    }

    /// i728 is-wired check — repo_keys of aggregates with NO persistence
    /// wiring, i.e. would be UNWIRED. An aggregate is WIRED when EITHER its
    /// governing hecksagon declares a legacy persistence adapter
    /// (`hex.persistence.is_some()` — the `adapter :heki` block, wiring by
    /// CONTEXT) OR it carries a resolved persistence-family binding
    /// (`Pizzas::Order.persisted_by("Heki")` — the hexagon port-verb surface,
    /// wiring by AGGREGATE FQN). Reading BOTH surfaces keeps this check in
    /// step with `apply_hexagon_persistence` (the backend resolver) : every
    /// aggregate the resolver rebinds is reported wired, never a false boot
    /// error. In Phase C a STRICT `.world` turns a non-empty result into a
    /// boot error ; here it is plumbing only — no caller enforces it, and
    /// `boot_in_memory` (the behaviors harness) bypasses it.
    pub fn unwired_aggregates(&self) -> Vec<String> {
        use super::hexagon_resolution::{resolve_bindings, ResolveOutcome};
        // Legacy `adapter :heki` block — wires by CONTEXT (whole hecksagon).
        let wired_contexts: std::collections::HashSet<&str> = self
            .hecksagons
            .iter()
            .filter(|hex| hex.persistence.is_some())
            .map(|hex| hex.name.as_str())
            .collect();
        // Port-verb binding `persisted_by(...)` — wires by AGGREGATE FQN. The
        // bind's aggregate IS the FQN, which equals `repo_key(context, name)`.
        // Mirror apply_hexagon_persistence's filter so the wired check and the
        // backend resolver read the same surface and never diverge.
        let wired_keys: std::collections::HashSet<String> = resolve_bindings(&self.hecksagons)
            .into_iter()
            .filter_map(|r| match r.outcome {
                ResolveOutcome::Resolved { family } if family == "persistence" => Some(r.aggregate),
                _ => None,
            })
            .collect();
        let mut out: Vec<String> = self
            .domain
            .aggregates
            .iter()
            .filter(|agg| {
                let ctx_unwired = agg
                    .context
                    .as_deref()
                    .is_none_or(|ctx| !wired_contexts.contains(ctx));
                let key = repo_key(agg.context.as_deref(), &agg.name);
                ctx_unwired && !wired_keys.contains(&key)
            })
            .map(|agg| repo_key(agg.context.as_deref(), &agg.name))
            .collect();
        out.sort();
        out
    }

    /// Sweep every Repository and reload it from disk if its heki
    /// file has been written by a sibling process since our last
    /// load or save. The kernel-floor implementation of the
    /// `RefreshOnPulse` policy declared in
    /// runtime/storage/storage.bluebook : LoopDriver calls this at
    /// the start of every tick so a long-running daemon's in-memory
    /// store stays current with writes from sibling processes
    /// (e.g. `storehouse sleep` dispatching EnterSleep against a
    /// heki the run-loop daemon will read on its next tick).
    ///
    /// **Opt-in via `HECKS_REFRESH_REPOS=1`** — refresh is off by
    /// default. Production daemons (mindstream / long-running
    /// run-loops) set the env var to pick up cross-process state.
    /// Single-process smoke tests and one-shot dispatches leave it
    /// off so refresh doesn't interact with their in-memory cascade
    /// state (e.g. by re-reading partially-written counter-minted
    /// records and mid-cascade breaking singleton fallback ; see
    /// dream_content_smoke flakiness 2026-05-09).
    ///
    /// Cost when on : one stat() per repo per tick when nothing
    /// changed ; per-repo refresh_from_heki gates the actual read
    /// on mtime advance.
    /// Cost when off : zero — the function returns immediately.
    /// Closes the i517 root cause for the production-daemon path.
    pub fn refresh_repositories_from_heki(&mut self) {
        if std::env::var("HECKS_REFRESH_REPOS").ok().as_deref() != Some("1") {
            return;
        }
        for repo in self.repositories.values_mut() {
            repo.refresh_from_heki();
        }
    }

    /// Warm-serve freshness sweep — refresh ONLY the repos already
    /// hydrated in this resident process. Sibling to
    /// `refresh_repositories_from_heki`, with two deliberate
    /// differences that make it the right primitive for `serve` mode :
    ///
    ///   1. **No env gate.** `serve` calls this unconditionally before
    ///      every dispatch. The `HECKS_REFRESH_REPOS=1` guard on the
    ///      sibling exists to keep one-shot CLI dispatches and in-memory
    ///      smoke tests from re-reading mid-cascade ; a resident server
    ///      that answers from a warm runtime must ALWAYS reconcile with
    ///      disk first, because daemons (heart/breath) write `.heki`
    ///      concurrently between requests.
    ///
    ///   2. **Hydrated-only.** `LazyRepository::refresh_from_heki` forces
    ///      `repo_mut()` → `get_or_init` → hydration. Sweeping ALL repos
    ///      would hydrate every aggregate on the first request and throw
    ///      away the lazy-boot win this whole feature is built on. We
    ///      filter on `is_hydrated()` : a repo that's never been touched
    ///      stays cold (and, when it IS first touched by a later
    ///      dispatch, the OnceCell init reads current disk by
    ///      definition — so cold repos are fresh for free). Only the
    ///      handful of repos this process has actually served pay the
    ///      one `stat()` per request ; the mtime gate inside
    ///      `refresh_from_heki` skips the re-read when disk is unchanged.
    ///
    /// This is THE correctness crux of warm serve : the IR stays warm
    /// (the boot is paid once) but the touched aggregate's STATE is
    /// never stale.
    pub fn refresh_hydrated_repositories_from_heki(&mut self) {
        for repo in self.repositories.values_mut() {
            if repo.is_hydrated() {
                repo.refresh_from_heki();
            }
        }
    }

}
