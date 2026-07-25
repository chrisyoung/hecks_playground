//! LoopDriver — continuous-tick PM scheduler
//!
//! The runtime daemon. Boots once, holds a Runtime, ticks at a
//! configured cadence, fires events / dispatches commands that drive
//! process_managers + policies forward without the per-tick boot cost
//! the shell `while true ; do storehouse ... ; sleep 1 ; done` pattern
//! pays.
//!
//! Substrate for retiring `mindstream.sh` : that shell exists because
//! no Rust scheduler does. With LoopDriver, the cadence becomes Rust
//! and the bluebook-declared PMs (SleepCycle, Dream, Mind) react to
//! each tick's events through the same machinery that runs in the
//! one-shot dispatch path.
//!
//! Usage :
//!   let mut driver = LoopDriver::new(runtime, Duration::from_secs(1));
//!   driver.add_dispatch("Pulse.Emit", attrs);
//!   driver.run();   // blocks until interrupted
//!
//! [antibody-exempt: rust/src/runtime/loop_driver.rs — kernel-floor
//!  scheduler. Honors the LoopDriver aggregate declared in
//!  runtime/loop_driver/loop_driver.bluebook (Tick command + lifecycle ;
//!  RegisterAction / RegisterBootstrap / Stop). The cadence bluebooks
//!  (body_tick / heart_tick / breath_tick) declare WHICH cadences
//!  exist ; LoopDriver declares HOW each tick orchestrates. Mirror
//!  of policy_engine.rs / pm_engine.rs in scope : generic interpreter
//!  for declared cadences. Retires alongside mindstream.sh once the
//!  block_grammar primitives land and a future LoopRunner reads
//!  cadences declaratively.
//!  i223 (2026-05-02) — adds BootstrapEmit : a one-shot first-tick
//!  synthetic-event injection gated on an aggregate-state predicate.
//!  Closes the mind-pm-bootstrap-on-attentive-restart gap.
//!  i517 (2026-05-09) — adds the freshness sweep at the start of
//!  each tick (refresh_repositories_from_heki). Honors the
//!  storage.bluebook RefreshOnPulse policy ; the loop's in-memory
//!  view of state stays current with disk between processes.]
//!
//! Time : `std::thread::sleep`. The runtime is sync ; bringing in tokio
//! is a much larger choice. Sync sleep gives us 1Hz cadence with
//! sub-millisecond drift across reasonable runs ; if drift matters,
//! switch to a deadline-based scheduler in a follow-up.
//!
//! Shutdown : the loop checks an `AtomicBool` between ticks. Callers
//! can flip the flag from a signal handler (or from a test). PM state
//! is persisted on every transition by the existing `drain_policies`
//! cascade in `runtime/mod.rs`, so even hard-kill loses no PM state —
//! the worst case is replaying one tick's policy cascade.

use super::{Runtime, Value};
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

/// Resolve `{now}` / `{now+N}` / `{now-N}` clock tokens in a dispatched
/// attr map (the cadence-loop write path, mirroring interpolate_event on
/// the hecksagon write path). Only string values are scanned ; the
/// resolver is a no-op for any value without a `{now` token, so the
/// per-tick cost is one substring check per string attr. HECKS_NOW pins
/// the base instant for deterministic tests.
pub(super) fn resolve_now_attrs(attrs: HashMap<String, Value>) -> HashMap<String, Value> {
    attrs
        .into_iter()
        .map(|(k, v)| match v {
            Value::Str(s) => (
                k,
                Value::Str(crate::runtime::storehouse_log::interpolate_now_tokens(&s)),
            ),
            other => (k, other),
        })
        .collect()
}

/// One scheduled action per tick. Either a synthetic event (drives
/// PMs only, no command pipeline) or a command dispatch (full
/// pipeline : givens → mutations → emit → cascade).
#[derive(Debug, Clone)]
pub enum TickAction {
    Emit {
        event_name: String,
        aggregate_type: String,
        aggregate_id: String,
        data: HashMap<String, Value>,
    },
    Dispatch {
        command_name: String,
        attrs: HashMap<String, Value>,
    },
}

/// One-shot bootstrap on the FIRST tick — fires the embedded `event`
/// only when the named aggregate's named field equals the expected
/// value. Closes the i223 gap : a daemon restarting while
/// `Consciousness.state == "attentive"` never sees a fresh `WokenUp`,
/// so PMs that `starts_on "WokenUp"` (Mind, future ones) are stranded
/// — no instance, the wake handler is inert. The bootstrap injects a
/// synthetic WokenUp on first tick when the state predicate matches,
/// putting Mind PM's instance into existence without requiring a
/// sleep/wake cycle.
///
/// The `event` field is a `TickAction::Emit` ; we reuse the variant
/// instead of a fresh struct so the bootstrap path runs through the
/// same `publish_synthetic_event` plumbing as a normal emit.
#[derive(Debug, Clone)]
pub struct BootstrapEmit {
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub field: String,
    pub expected: String,
    pub event: TickAction,
}

pub struct LoopDriver {
    pub(super) runtime: Runtime,
    pub(super) interval: Duration,
    pub(super) actions: Vec<TickAction>,
    /// i223 — bootstraps fire ONCE on the first tick when their
    /// state predicate matches. Drained after the first tick so the
    /// emission can never replay.
    pub(super) bootstraps: Vec<BootstrapEmit>,
    pub(super) stop_flag: Arc<AtomicBool>,
    pub(super) tick_count: u64,
}

impl LoopDriver {
    pub fn new(runtime: Runtime, interval: Duration) -> Self {
        Self {
            runtime,
            interval,
            actions: Vec::new(),
            bootstraps: Vec::new(),
            stop_flag: Arc::new(AtomicBool::new(false)),
            tick_count: 0,
        }
    }

    /// i223 — register a one-shot bootstrap. Fires once on the first
    /// tick if `aggregate.field == expected` ; the embedded event is
    /// then injected via `publish_synthetic_event`. After the first
    /// tick the bootstrap is drained, regardless of whether the
    /// predicate matched (predicate is evaluated against state at
    /// boot time ; later transitions will produce real WokenUp events
    /// through the regular dispatch path).
    pub fn add_bootstrap(&mut self, bootstrap: BootstrapEmit) {
        self.bootstraps.push(bootstrap);
    }

    /// Stop flag — flip from a signal handler or test thread to break
    /// the run loop on the next tick boundary.
    pub fn stop_flag(&self) -> Arc<AtomicBool> {
        self.stop_flag.clone()
    }

    pub fn add_action(&mut self, action: TickAction) {
        self.actions.push(action);
    }

    pub fn add_emit(
        &mut self,
        event_name: &str,
        aggregate_type: &str,
        aggregate_id: &str,
        data: HashMap<String, Value>,
    ) {
        self.add_action(TickAction::Emit {
            event_name: event_name.to_string(),
            aggregate_type: aggregate_type.to_string(),
            aggregate_id: aggregate_id.to_string(),
            data,
        });
    }

    pub fn add_dispatch(&mut self, command_name: &str, attrs: HashMap<String, Value>) {
        self.add_action(TickAction::Dispatch {
            command_name: command_name.to_string(),
            attrs,
        });
    }

    /// Borrow the inner runtime — tests inspect PM state through this.
    pub fn runtime(&self) -> &Runtime { &self.runtime }
    pub fn runtime_mut(&mut self) -> &mut Runtime { &mut self.runtime }

    pub fn tick_count(&self) -> u64 { self.tick_count }

}
