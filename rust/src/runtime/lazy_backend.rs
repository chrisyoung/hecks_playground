//! lazy_backend — the LazyRepository hydration cell : hydrate-on-first-
//! access accessors (repo / repo_mut via OnceCell, adapter / adapter_mut)
//! and the backend introspection surface (is_adapter, backend_kind,
//! heki_path). Constructors + the op facade live in lazy_repository.rs /
//! lazy_repo_ops.rs.
//!
//! Cask extracted VERBATIM from runtime/lazy_repository.rs (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/lazy_backend.rs — kernel-floor i-lazy
//!  hydration, relocated verbatim from lazy_repository.rs blanket.]

use super::lazy_repository::{Backend, BackendKind, LazyRepository};
use super::persistence_adapter::PersistenceAdapter;
use super::repository::Repository;

impl LazyRepository {
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
