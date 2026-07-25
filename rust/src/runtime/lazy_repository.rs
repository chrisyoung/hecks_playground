//! LazyRepository — deferred-hydration wrapper around Repository
//!
//! [antibody-exempt: rust/src/runtime/lazy_repository.rs — kernel-floor
//!  runtime perf ; OnceCell wrapper deferring Repository::load_persisted
//!  to first access. Same kernel-surface concern as sibling
//!  repository.rs ; a bluebook can't describe its storage substrate's
//!  boot timing.]
//!
//! Boot used to construct + hydrate ALL aggregate repositories eagerly
//! (Repository::new_with_context calls load_persisted in its
//! constructor, which does up to 2x heki::read per aggregate). With
//! 458 aggregates in the full conception that cost ~4.2s at boot — the
//! dominant share of a ~5.3s cold single-shot dispatch — even though a
//! single dispatch touches exactly ONE repository.
//!
//! LazyRepository holds the construction parameters and a
//! `OnceCell<Repository>`. The real Repository (and its load_persisted
//! disk read) is materialised on FIRST access, not at boot. A
//! single-shot dispatch hydrates one repo ; a long-running daemon
//! hydrates each repo on first touch and keeps it warm thereafter —
//! same end-state, the cost is paid lazily and only for what's used.
//!
//! The forwarding methods mirror Repository's surface so existing call
//! sites (`repo.find(id)`, `repo.save(...)`, `repo.all()`, ...) work
//! unchanged whether they hold `&LazyRepository` or `&mut
//! LazyRepository`. Read methods route through `OnceCell::get_or_init`
//! (takes `&self`) so the `Runtime::find`/`Runtime::all`/query paths
//! that borrow `&self` still hydrate transparently — no `&self`->`&mut
//! self` cascade up through the runtime.
//!
//! `Repository::new_with_context`'s own eager semantics are unchanged
//! (anything that constructs a Repository directly still hydrates in
//! the constructor) — laziness lives only at this boot-map layer.
//!
//! Usage:
//!   let lazy = LazyRepository::new("Heartbeat", data_dir, Some("name".into()), None);
//!   let state = lazy.find("heartbeat");      // hydrates on first call
//!   lazy_mut.save(state, ctx);               // hydrates if not yet


use super::repository::Repository;
use super::persistence_adapter::PersistenceAdapter;
#[cfg(test)]
use super::AggregateState;
#[cfg(test)]
use crate::heki;
use std::cell::OnceCell;

/// Which storage substrate a repository wraps. Chosen at construction
/// from the hecksagon's `persistence` declaration — heki/memory is the
/// default ; `adapter :sqlite, db:` selects Sql. Heki/memory defer their
/// first disk touch to first access via a OnceCell ; Sql is EAGER (built
/// at boot) because its construction is fallible (i735 defect 2).
pub(super) enum Backend {
    /// The explicit in-memory adapter (`adapter :memory`). A pure in-process
    /// HashMap keyed by aggregate id, alive for the process, gone on restart.
    /// Distinct from `Heki { data_dir: None }` : memory is a wired CHOICE, not
    /// an implicit fallback. Reuses `Repository` with `data_dir = None` so every
    /// disk branch is skipped — no heki path is ever reached.
    Memory {
        aggregate_type: String,
        identified_by: Option<String>,
        context: Option<String>,
        cell: OnceCell<Repository>,
    },
    Heki {
        aggregate_type: String,
        data_dir: Option<String>,
        identified_by: Option<String>,
        context: Option<String>,
        cell: OnceCell<Repository>,
    },
    /// The AppendLog adapter (`persisted_by("AppendLog")`) — the bluebook-first
    /// Event Log, which OWNS its append-only substrate end-to-end (heki is for
    /// snapshots, the LOG is append-only — Chris, 2026-06-21). SAVE appends one
    /// immutable record to THIS process's private shard (single-writer,
    /// lossless), folded into the global Log by the merge. READ seeds the
    /// Repository from the append-only JSONL global Log (`event.log`,
    /// event_log.rs) — NOT a heki store — with `data_dir = None` so the generic
    /// heki `load_persisted` is a no-op ; heki never sees the Log file.
    AppendLog {
        aggregate_type: String,
        data_dir: Option<String>,
        identified_by: Option<String>,
        context: Option<String>,
        cell: OnceCell<Repository>,
        /// Last-seen mtime of the append-only `event.log`. The reader re-seeds
        /// (invalidates `cell`) only when the Log has GROWN since — so a refresh
        /// is a cheap stat until the merge actually appends. Cell so `repo()`
        /// (&self) can stamp it on first materialization.
        log_mtime: std::cell::Cell<Option<std::time::SystemTime>>,
    },
    /// A WIRED adapter (`persisted_by("Sqlite")`, future R2 / postgres) — a
    /// heavy / often native-only backend living in its OWN crate, held behind
    /// the `PersistenceAdapter` port so the kernel names no concrete engine.
    /// EAGER, unlike the lazy heki/memory cells : built at boot by
    /// `apply_wired_adapters` because construction is FALLIBLE (i735 defect 2)
    /// and the infallible forwarded read surface (`find`/`save`) cannot host a
    /// deferred `Result`. A failed build refuses the aggregate at boot, never
    /// silently swaps to heki.
    Adapter {
        adapter: Box<dyn PersistenceAdapter>,
    },
}

/// The persistence backend a `LazyRepository` resolved to — the read-only
/// discriminant `backend_kind()` exposes for the i728 backend-map gate (the
/// Phase-A enforcement check that no production domain silently changes backend).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    Heki,
    Memory,
    Sql,
    AppendLog,
}

pub struct LazyRepository {
    pub(super) backend: Backend,
}

impl LazyRepository {
    /// Construct a heki/memory-backed lazy wrapper. Stores the params ;
    /// does NOT touch disk. The first access materialises the real
    /// Repository via `Repository::new_with_context`, which runs
    /// `load_persisted` then.
    pub fn new(
        aggregate_type: &str,
        data_dir: Option<String>,
        identified_by: Option<String>,
        context: Option<String>,
    ) -> Self {
        LazyRepository {
            backend: Backend::Heki {
                aggregate_type: aggregate_type.to_string(),
                data_dir,
                identified_by,
                context,
                cell: OnceCell::new(),
            },
        }
    }

    /// Construct the AppendLog adapter (`persisted_by("AppendLog")`). Same
    /// params + lazy cell as `new` (heki) — it READS from `data_dir` (the
    /// merged event.heki) identically ; only `save` diverges to append-to-shard.
    /// The shard dir is a `shards/` sibling of `data_dir`, resolved in `save`.
    pub fn new_appendlog(
        aggregate_type: &str,
        data_dir: Option<String>,
        identified_by: Option<String>,
        context: Option<String>,
    ) -> Self {
        LazyRepository {
            backend: Backend::AppendLog {
                aggregate_type: aggregate_type.to_string(),
                data_dir,
                identified_by,
                context,
                cell: OnceCell::new(),
                log_mtime: std::cell::Cell::new(None),
            },
        }
    }

    /// Construct the explicit in-memory adapter (`adapter :memory`). A pure
    /// in-process HashMap keyed by aggregate id ; survives the process, vanishes
    /// on restart. The materialised Repository carries `data_dir = None`, so no
    /// disk read/write is ever performed. Distinct from `new(.., None, ..)` :
    /// memory is a deliberate, wired choice, not an implicit fallback.
    pub fn new_memory(
        aggregate_type: &str,
        identified_by: Option<String>,
        context: Option<String>,
    ) -> Self {
        LazyRepository {
            backend: Backend::Memory {
                aggregate_type: aggregate_type.to_string(),
                identified_by,
                context,
                cell: OnceCell::new(),
            },
        }
    }

    /// Wrap an ALREADY-constructed `SqliteRepository`. Unlike the heki/
    /// memory path this is EAGER : the repo (open + CREATE TABLE +
    /// row-load) is built by the caller (`apply_sqlite_persistence`) so a
    /// fallible table-create is decided at boot, not deferred behind a
    /// OnceCell the infallible read surface can't fail through.
    pub fn new_adapter(adapter: Box<dyn PersistenceAdapter>) -> Self {
        LazyRepository {
            backend: Backend::Adapter { adapter },
        }
    }

    /// Hydrate-on-first-access for the heki backend. `OnceCell::get_or_init`
    /// takes `&self`, so this is callable from `&self` runtime paths.
    pub(super) fn repo(&self) -> &Repository {
        match &self.backend {
            Backend::Memory { aggregate_type, identified_by, context, cell } => {
                cell.get_or_init(|| {
                    // data_dir = None → pure in-memory ; no disk branch is reached.
                    Repository::new_with_context(
                        aggregate_type,
                        None,
                        identified_by.clone(),
                        context.clone(),
                    )
                })
            }
            Backend::Heki { aggregate_type, data_dir, identified_by, context, cell } => {
                    cell.get_or_init(|| {
                        Repository::new_with_context(
                            aggregate_type,
                            data_dir.clone(),
                            identified_by.clone(),
                            context.clone(),
                        )
                    })
                }
            Backend::AppendLog { aggregate_type, data_dir, identified_by, context, cell, log_mtime } => {
                    cell.get_or_init(|| {
                        // AppendLog OWNS its substrate : seed the Repository from the
                        // append-only JSONL global Log (event_log), NOT the generic
                        // heki load. data_dir = None so the Repository's heki
                        // load_persisted is a no-op ; we seed it from the Log
                        // ourselves, so heki never sees the Log file.
                        let mut r = Repository::new_with_context(
                            aggregate_type,
                            None,
                            identified_by.clone(),
                            context.clone(),
                        );
                        if let Some(dir) = data_dir {
                            let path = super::event_log::global_path(dir, context.as_deref());
                            log_mtime.set(super::event_log::mtime(&path));
                            for st in super::event_log::load_states(&path) {
                                r.seed_record(st);
                            }
                        }
                        r
                    })
                }
            Backend::Adapter { .. } => unreachable!("repo() on a SQL-backed LazyRepository"),
        }
    }

    /// The wired adapter behind this repository (`Backend::Adapter`). The
    /// kernel reaches a wired backend ONLY through the `&dyn PersistenceAdapter`
    /// port — it never names a concrete engine.
    pub(super) fn adapter(&self) -> &dyn PersistenceAdapter {
        match &self.backend {
            Backend::Adapter { adapter } => adapter.as_ref(),
            Backend::Heki { .. } | Backend::Memory { .. } | Backend::AppendLog { .. } => {
                unreachable!("adapter() on a non-adapter LazyRepository")
            }
        }
    }

    /// True when this wrapper is SQL-backed (selected by `adapter
    /// :sqlite`). Read methods branch on it to route to the right cell ;
    /// `apply_sqlite_persistence`'s per-context scoping test asserts on it
    /// (i735) to prove only the declaring domain's repos became SQL.
    pub fn is_adapter(&self) -> bool {
        matches!(self.backend, Backend::Adapter { .. })
    }

    /// The backend variant this repository resolved to, WITHOUT hydrating the
    /// OnceCell — a `&self` peek at the enum tag. Feeds `Runtime::dump_backend_map`,
    /// the i728 Phase-A gate asserting no production domain silently changes backend.
    pub fn backend_kind(&self) -> BackendKind {
        match &self.backend {
            Backend::Heki { .. } => BackendKind::Heki,
            Backend::Memory { .. } => BackendKind::Memory,
            Backend::Adapter { .. } => BackendKind::Sql,
            Backend::AppendLog { .. } => BackendKind::AppendLog,
        }
    }

    /// The heki store dir this repository is rooted at, when heki-backed. `None`
    /// for memory (no disk) and sql (its own db path). Peeks without hydrating.
    pub fn heki_path(&self) -> Option<String> {
        match &self.backend {
            Backend::Heki { data_dir, .. } | Backend::AppendLog { data_dir, .. } => data_dir.clone(),
            _ => None,
        }
    }

    /// Mutable hydrate-on-first-access (heki). Forces the cell via the
    /// `&self` initialiser then hands back the `&mut`.
    pub(super) fn repo_mut(&mut self) -> &mut Repository {
        let _ = self.repo();
        match &mut self.backend {
            Backend::Heki { cell, .. } | Backend::Memory { cell, .. } | Backend::AppendLog { cell, .. } => {
                cell.get_mut().expect("cell initialised by repo() above")
            }
            Backend::Adapter { .. } => unreachable!("repo_mut() on a SQL-backed LazyRepository"),
        }
    }

    /// Mutable access to the wired adapter. Mirror of `adapter`. Eager, so
    /// no OnceCell pre-hydration is needed (unlike `repo_mut`).
    pub(super) fn adapter_mut(&mut self) -> &mut dyn PersistenceAdapter {
        match &mut self.backend {
            Backend::Adapter { adapter } => adapter.as_mut(),
            Backend::Heki { .. } | Backend::Memory { .. } | Backend::AppendLog { .. } => {
                unreachable!("adapter_mut() on a non-adapter LazyRepository")
            }
        }
    }


}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_backend_retains_within_process_and_vanishes_on_restart() {
        // The explicit in-memory stub : a HashMap keyed by aggregate id, alive
        // for the process, gone on restart. No disk is ever touched.
        let mut repo = LazyRepository::new_memory("Order", Some("id".into()), None);
        repo.save(
            AggregateState::new("order-1"),
            heki::WriteContext::OutOfBand { reason: "test" },
        );
        // Stored and retrievable by id within the process.
        assert!(repo.find("order-1").is_some());
        assert_eq!(repo.count(), 1);

        // A fresh memory repository (process "restart") starts empty — no disk
        // means nothing survives the new instance.
        let fresh = LazyRepository::new_memory("Order", Some("id".into()), None);
        assert!(fresh.find("order-1").is_none());
        assert_eq!(fresh.count(), 0);
    }
}
