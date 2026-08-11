//! persistence_apply — the boot-time persistence wiring : 
//! apply_memory_persistence (`:memory` hecksagon → in-process Backend::
//! Memory, per-context scoping, host- AND wasm-valid) and
//! apply_hexagon_persistence (the persisted_by consult — wired adapters
//! replace the default repo, unwired production fails loud). Inspection +
//! refresh + the test-memory switch stay in persistence_resolution.rs.
//!
//! Cask extracted VERBATIM from runtime/persistence_resolution.rs
//! (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/persistence_apply.rs — kernel-floor
//!  persistence wiring, relocated verbatim from persistence_resolution.rs
//!  blanket.]

use super::*;

impl Runtime {
    /// Rebuild on the explicit `Backend::Memory` ONLY those aggregates whose
    /// governing hecksagon declares `:memory` — matched by `agg.context ==
    /// hecksagon.name`, the same per-context scoping as
    /// `apply_sqlite_persistence`. Until this ran, `:memory` was INERT : a
    /// declared-`:memory` aggregate fell through to the implicit heki default
    /// `boot_with_data_dir` built, so `:memory` and `unwired` were
    /// indistinguishable on disk. Now `:memory` actually selects in-process
    /// memory — the keystone for "tests get memory, production fails loud on
    /// unwired". Every other aggregate keeps its heki repository. A no-op when
    /// no `:memory` hecksagon is attached. Memory is host- AND wasm-valid, so
    /// — unlike `apply_sqlite_persistence` — this carries no cfg gate.
    pub(super) fn apply_memory_persistence(&mut self) {
        let memory_contexts: std::collections::HashSet<&str> = self
            .hecksagons
            .iter()
            .filter(|hex| hex.persistence.as_deref() == Some("memory"))
            .map(|hex| hex.name.as_str())
            .collect();
        if memory_contexts.is_empty() {
            return;
        }
        // Collect patches under the immutable borrow of self.domain, then apply
        // them under the mutable borrow of self.repositories — mirrors
        // apply_sqlite_persistence's two-phase borrow discipline.
        let patches: Vec<(String, String, Option<String>, Option<String>)> = self
            .domain
            .aggregates
            .iter()
            .filter(|agg| {
                agg.context
                    .as_deref()
                    .is_some_and(|ctx| memory_contexts.contains(ctx))
            })
            .map(|agg| {
                (
                    repo_key(agg.context.as_deref(), &agg.name),
                    agg.name.clone(),
                    agg.identified_by.clone(),
                    agg.context.clone(),
                )
            })
            .collect();
        for (key, name, identified_by, context) in patches {
            self.repositories
                .insert(key, LazyRepository::new_memory(&name, identified_by, context));
        }
    }
    /// Bucket-3 step 4 — the hexagon-binding CONSULT. Where
    /// `apply_memory_persistence` / `apply_sqlite_persistence` read the LEGACY
    /// `.hecksagon` persistence block (`hex.persistence`), this reads the NEW
    /// port-verb binding surface — `Pizzas::Order.persisted_by("Heki")` — that
    /// steps 1-3 parse + resolve. It runs `resolve_bindings` over the loaded IR
    /// and, for every bind that resolves on the `persistence` family, rebuilds
    /// that aggregate's repository on the backend its adapter NAMES — so the
    /// runtime OBEYS the declaration rather than coinciding with the heki
    /// default. Additive : an aggregate with no persistence binding keeps the
    /// repository `boot_with_data_dir` built. Runs AFTER apply_memory /
    /// apply_sqlite, so where an aggregate ever carried BOTH a legacy block and
    /// a binding the binding (the new surface) is last-writer ; no corpus
    /// aggregate carries both today.
    ///
    /// adapter-name → backend : "Heki" rebuilds heki at the runtime `data_dir`
    /// — the SAME constructor `boot_with_data_dir` uses, so it is byte-identical
    /// to the default, now binding-driven ; "Memory" selects the explicit
    /// in-process `Backend::Memory` (the discriminating case that proves the
    /// consult bites). Sqlite via the binding surface is not exercised —
    /// `adapter :sqlite, db:` still routes through `apply_sqlite_persistence`'s
    /// legacy block (fallible, db-path-carrying), so an unrecognised
    /// persistence-family adapter leaves the default repo untouched. Ungated :
    /// like memory, the heki/memory constructors are host- AND wasm-valid.
    pub(super) fn apply_hexagon_persistence(&mut self) {
        use super::hexagon_resolution::{resolve_bindings, ResolveOutcome};
        // (aggregate FQN, adapter) for every bind resolving on the persistence
        // family. resolve_bindings already type-checked the adapter->family->verb
        // attach ; an unresolved bind never reaches here.
        let binds: Vec<(String, String)> = resolve_bindings(&self.hecksagons)
            .into_iter()
            .filter_map(|r| match r.outcome {
                ResolveOutcome::Resolved { family } if family == "persistence" => {
                    Some((r.aggregate, r.adapter))
                }
                _ => None,
            })
            .collect();
        if binds.is_empty() {
            return;
        }
        let data_dir = self.data_dir.clone();
        // Resolve each bound FQN to its repo_key + ctor params under the
        // immutable borrow ; patch under the mutable borrow — the two-phase
        // discipline apply_memory_persistence uses. The bind's aggregate IS the
        // FQN, which equals `repo_key(context, name)`.
        // (aggregate, family, adapter, engine, verb, target) — one resolved
        // persistence binding waiting to be applied to a repository.
        type PersistencePatch =
            (String, String, Option<String>, Option<String>, String, Option<String>);
        let patches: Vec<PersistencePatch> = self
            .domain
            .aggregates
            .iter()
            .filter_map(|agg| {
                let key = repo_key(agg.context.as_deref(), &agg.name);
                binds.iter().find(|(fqn, _)| *fqn == key).map(|(_, adapter)| {
                    (
                        key.clone(),
                        agg.name.clone(),
                        agg.identified_by.clone(),
                        agg.context.clone(),
                        adapter.clone(),
                        agg.realm_path.clone(),
                    )
                })
            })
            .collect();
        for (key, name, identified_by, context, adapter, realm_path) in patches {
            // Realm-anchor the Heki / AppendLog store on the aggregate's OWN
            // realm, not the host runtime's global data_dir — the same
            // resolution boot_with_data_dir uses, so a cross-corpus aggregate
            // (e.g. a miette bluebook loaded by the hecks runtime) stays on
            // its own chain instead of the host's.
            let agg_dir = crate::heki::realm_store_dir(realm_path.as_deref(), data_dir.as_deref());
            let repo = match adapter.as_str() {
                "Memory" => LazyRepository::new_memory(&name, identified_by, context),
                "Heki" => LazyRepository::new(&name, agg_dir, identified_by, context),
                // AppendLog — the bluebook-first Event Log. READS from the same
                // data_dir as Heki (the merged event.heki) ; SAVE appends to a
                // per-process shard. Same ctor signature as Heki.
                "AppendLog" => LazyRepository::new_appendlog(&name, agg_dir, identified_by, context),
                // A persistence-family adapter the consult does not mint (e.g. a
                // future Sqlite binding) leaves the default repo in place.
                _ => continue,
            };
            self.repositories.insert(key, repo);
        }
    }}
