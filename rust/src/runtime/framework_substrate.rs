//! Framework substrate — the kernel's own aggregates, and how it reaches them.
//!
//! [antibody-exempt: rust/src/runtime/framework_substrate.rs — kernel-floor
//!  runtime, extracted verbatim from runtime/mod.rs (which carries the same
//!  marker). The collaborator that holds the runtime's OWN aggregates cannot
//!  itself be a bluebook : it is the mechanism by which bluebook substrate is
//!  reached at all.]
//!
//! The runtime must write aggregates the USER's domain does not own : the event
//! Log (`EventSourcing::Event`), the veto audit (`Governance::Violation`) and
//! the event-out outbox (`OutboundEvent`). It used to ask "is that aggregate in
//! MY domain?" and silently do nothing when it wasn't — a minimal boot wrote no
//! Log and lost authorization audit rows — while a graft spliced the outbox into
//! any domain with an effect port to paper over the third.
//!
//! The answer is a COLLABORATOR : a second Runtime booted on crate-owned
//! substrate that the kernel dispatches into. This module owns it — the embedded
//! bluebooks, the merged framework domain, the declared store location, the lazy
//! accessor, the inspection surface, and the DOOR-side lookup an out-of-process
//! caller needs.
//!
//! `CascadeRun` deliberately does NOT live here : its guard selects between two
//! WORKING paths (persistent outbox vs the in-memory pump), not between working
//! and losing data. See `inbox/CARD-framework-substrate-service.md`.
//!
//! Inherent `impl Runtime` methods in a child module, reaching Runtime's private
//! fields through child-module privacy and the `super::*` glob — the same shape
//! `query.rs` and `reaction.rs` use.

use super::*;

impl Runtime {
    /// The embedded framework outbox bluebook — the OutboundEvent event-out
    /// port, compiled in as runtime standard-library substrate so any booted
    /// domain with an effect binding gets it without copying. This is ENGINE
    /// stdlib (the messaging-port analog of CascadeRun), so the canonical copy
    /// is CRATE-OWNED (`rust/resources/outbound_event.bluebook`) — storehouse
    /// carries its own outbox contract and no longer reaches into a sibling
    /// `hecks_conception/` to build (decouple Phase 1). The conception keeps a
    /// copy (referenced by event_sourcing.bluebook) ; a monorepo parity check
    /// guards them against drift. Loaded into the framework collaborator, never
    /// merged into a user's domain.
    #[cfg(target_arch = "wasm32")]
    const OUTBOX_SUBSTRATE: &'static str = include_str!(
        "../../resources/outbound_event.bluebook"
    );

    /// The embedded EventSourcing chapter — the Event Log the kernel appends
    /// to. Crate-owned for the same reason the outbox is : storehouse must be
    /// able to event-source a domain without a sibling `hecks_conception/`.
    #[cfg(target_arch = "wasm32")]
    const EVENT_SOURCING_SUBSTRATE: &'static str = include_str!(
        "../../resources/event_sourcing.bluebook"
    );

    /// The embedded Governance chapter — the veto-audit sink every denied
    /// dispatch records to. Crate-owned for the same reason as the others : an
    /// authorization denial must be auditable on ANY runtime, not only one that
    /// happened to merge the Governance conception.
    #[cfg(target_arch = "wasm32")]
    const GOVERNANCE_SUBSTRATE: &'static str = include_str!(
        "../../resources/governance.bluebook"
    );

    /// Every framework substrate the collaborator carries, merged into one
    /// domain. Aggregates keep their own `context`, so FQN dispatch
    /// (`Governance::Violation.Record`, `EventSourcing::Event.Append`) resolves
    /// against the merge exactly as it would against a corpus.
    ///
    /// ONE collaborator, not one per substrate (CARD decision 1) : four boots
    /// would mean four stores and four times the wiring for a single mechanism.
    fn framework_domain() -> Domain {
        Self::merge_framework_sources(Self::framework_sources())
    }

    /// Merge an ordered list of framework bluebook sources into ONE domain.
    /// Aggregates keep their own `context` ; a later source never clobbers an
    /// earlier aggregate of the same (name, context). Shared by the native
    /// (disk) and wasm (embedded) source paths so the merge is identical.
    fn merge_framework_sources(sources: Vec<String>) -> Domain {
        let mut merged: Option<Domain> = None;
        for src in &sources {
            let extra = crate::parser::parse(src);
            match merged.as_mut() {
                None => merged = Some(extra),
                Some(domain) => {
                    for agg in extra.aggregates {
                        if !domain
                            .aggregates
                            .iter()
                            .any(|e| e.name == agg.name && e.context == agg.context)
                        {
                            domain.aggregates.push(agg);
                        }
                    }
                    domain.policies.extend(extra.policies);
                }
            }
        }
        merged.expect("at least one framework bluebook must be present")
    }

    /// The framework stdlib sources, in deterministic order. NATIVE reads every
    /// `.bluebook` in the engine-owned resources dir FROM DISK — the substrate is
    /// convention-loaded data, never baked into the generic binary. Sorted by
    /// path so the merge base is stable (event_sourcing, governance,
    /// outbound_event) — the same order the embedded list used.
    #[cfg(not(target_arch = "wasm32"))]
    fn framework_sources() -> Vec<String> {
        let dir = Self::framework_resources_dir().unwrap_or_else(|| {
            panic!(
                "framework resources dir not found — set STOREHOUSE_FRAMEWORK_DIR \
                 or run the engine inside the hecks repo"
            )
        });
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("read framework resources {}: {e}", dir.display()))
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "bluebook"))
            .collect();
        paths.sort();
        paths
            .iter()
            .map(|p| {
                std::fs::read_to_string(p)
                    .unwrap_or_else(|e| panic!("read framework bluebook {}: {e}", p.display()))
            })
            .collect()
    }

    /// WASM has no filesystem, so the edge artifact keeps the substrate EMBEDDED
    /// (the include_str! consts above). That is the specializer's job — a target
    /// that cannot read a disk bakes the domains it needs — not the generic
    /// runtime's default. Same three sources, same order as the native glob.
    #[cfg(target_arch = "wasm32")]
    fn framework_sources() -> Vec<String> {
        vec![
            Self::EVENT_SOURCING_SUBSTRATE.to_string(),
            Self::GOVERNANCE_SUBSTRATE.to_string(),
            Self::OUTBOX_SUBSTRATE.to_string(),
        ]
    }

    /// The engine-owned framework stdlib dir (`rust/resources/`), resolved from
    /// the EXECUTABLE — never from HECKS_CONCEPTION_DIR, which a test redirects to
    /// a temp conception. `STOREHOUSE_FRAMEWORK_DIR` overrides for packaging.
    #[cfg(not(target_arch = "wasm32"))]
    fn framework_resources_dir() -> Option<std::path::PathBuf> {
        if let Ok(d) = std::env::var("STOREHOUSE_FRAMEWORK_DIR") {
            if !d.is_empty() {
                return Some(std::path::PathBuf::from(d));
            }
        }
        crate::heki::walk_up_for_repo_root().map(|r| r.join("rust").join("resources"))
    }

    /// Every delivery currently in the framework outbox.
    ///
    /// The outbox is framework substrate and lives in the collaborator, so a
    /// caller inspecting it (a test, an operator tool) cannot reach it through
    /// its own domain any more — `rt.all("OutboundEvent")` returns nothing,
    /// correctly, because the user's domain never carried it. Empty when no
    /// delivery has ever been recorded (the collaborator has not booted).
    pub fn outbound_deliveries(&self) -> Vec<&AggregateState> {
        self.framework
            .as_ref()
            .map(|fw| fw.all("OutboundEvent"))
            .unwrap_or_default()
    }

    /// One delivery from the framework outbox, by delivery_id. The `find`
    /// sibling of `outbound_deliveries` — same reason it exists.
    pub fn outbound_delivery(&self, delivery_id: &str) -> Option<&AggregateState> {
        self.framework
            .as_ref()
            .and_then(|fw| fw.find("OutboundEvent", delivery_id))
    }

    /// Resolve a query against the framework collaborator (the outbox's
    /// `Pending` / `AllPending`, the Log, the veto audit). Returns an empty
    /// result shape when the collaborator has not booted.
    pub fn framework_query(
        &self,
        query_name: &str,
        attrs: &std::collections::HashMap<String, String>,
    ) -> serde_json::Value {
        match self.framework.as_ref() {
            Some(fw) => fw.resolve_query(query_name, attrs),
            None => serde_json::json!({ "state": [] }),
        }
    }

    /// The framework collaborator, booted on first use.
    ///
    /// This is the step-zero proof for CARD-framework-substrate-service : the
    /// kernel gets somewhere to dispatch framework commands that is NOT the
    /// user's domain. Booted lazily because most runtimes never append an
    /// Event, and paying a 400-line parse at every boot would tax all 124 test
    /// binaries for a path they don't take.
    ///
    /// It inherits `data_dir` so the Log lands under the same store root the
    /// parent writes to. (CARD decision 4 says the durable home is the
    /// FRAMEWORK REALM ; inheriting the parent's dir is the step-zero shape and
    /// is what keeps temp-dir tests isolated. Moving it to the framework realm
    /// is a follow-up, not a borrow-shape question.)
    ///
    /// The collaborator's own `framework` stays `None` — it never needs one,
    /// and that is what bounds the recursion.
    pub fn framework_mut(&mut self) -> &mut Runtime {
        if self.framework.is_none() {
            let dir = self.framework_store_dir();
            let rt = Runtime::boot_with_data_dir(Self::framework_domain(), dir);
            self.framework = Some(Box::new(rt));
        }
        self.framework.as_mut().expect("just booted")
    }

    /// Resolve the framework store location from its DECLARATION.
    ///
    /// `framework.world` states it : `dir :default` means "the same folder as the
    /// host application's persistence" (the inherited `data_dir`) ; a literal dir
    /// overrides. Read from the engine's resources dir ON DISK — no longer
    /// compiled in — so a missing declaration simply falls back to the default.
    /// Temp-dir isolation is preserved : `:default` resolves to whatever the host
    /// is using, so a `/tmp` conception keeps its own store.
    #[cfg(not(target_arch = "wasm32"))]
    fn framework_store_dir(&self) -> Option<String> {
        let world_src = Self::framework_resources_dir()
            .and_then(|d| std::fs::read_to_string(d.join("framework.world")).ok())
            .unwrap_or_default();
        let world = crate::world::parser::parse(&world_src);
        match world.config_for("heki").and_then(|c| c.get("dir")) {
            // The declared default : co-locate with the host's persistence.
            Some("default") | None => self.data_dir.clone(),
            // An explicit location wins over the host's.
            Some(literal) => Some(crate::heki::expand_tilde(literal)),
        }
    }

    /// wasm has no world parser (`crate::world::parser` is cfg'd out) and no
    /// filesystem to point a literal dir at, so the declaration cannot be read
    /// there. That costs nothing : `dir :default` MEANS "inherit the host's
    /// store", which is exactly what this returns. A wasm worker therefore
    /// behaves identically to a host running the shipped declaration — it just
    /// cannot honour an overridden one, which it could not reach anyway.
    #[cfg(target_arch = "wasm32")]
    fn framework_store_dir(&self) -> Option<String> {
        self.data_dir.clone()
    }

    /// Can the framework collaborator resolve `<aggregate>.<tail>` — as either a
    /// query or a command on one of the substrate aggregates it owns?
    ///
    /// This is the DOOR-side half of the collaborator. An out-of-process door
    /// (the CLI, the adapter host) resolves a verb against the SERVED domain ;
    /// framework aggregates are deliberately not in that domain any more, so
    /// without this the verb falls through and dies as an unknown command. That
    /// is exactly how `dream_content_smoke` broke : every IN-process caller was
    /// rerouted to the collaborator and no out-of-process one was, so the whole
    /// unit suite stayed green while a cross-process read failed.
    ///
    /// Consulted ONLY AFTER the served domain fails to resolve, so the common
    /// path never pays for booting the collaborator. Derived from the substrate
    /// domain itself — never a hardcoded name list, which would drift the moment
    /// a substrate is added or renamed.
    pub fn framework_resolves(&mut self, aggregate: &str, tail: &str) -> bool {
        self.framework_mut()
            .domain
            .aggregates
            .iter()
            .filter(|a| a.name == aggregate)
            .any(|a| {
                a.queries.iter().any(|q| {
                    crate::util::snake_case(&q.name) == tail || q.name == tail
                }) || a.commands.iter().any(|c| c.name == tail)
            })
    }
}
