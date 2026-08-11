//! loop_tick — the LoopDriver's execution half : run (sleep-interval
//! forever loop), run_ticks, tick_once (every action in order — emits,
//! dispatches, {now} resolution), fire_bootstrap (predicate-gated synthetic
//! event), and format_value_string (the predicate stringifier). The
//! builder/config surface stays in loop_driver.rs.
//!
//! Cask extracted VERBATIM from runtime/loop_driver.rs (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/loop_tick.rs — kernel-floor loop
//!  execution, relocated verbatim from loop_driver.rs blanket.]

use super::loop_driver::{resolve_now_attrs, BootstrapEmit, LoopDriver, TickAction};
use super::{Event, Value};
use std::sync::atomic::Ordering;
use std::time::Instant;

impl LoopDriver {
    /// Run forever (or until `stop_flag` is set). Sleeps `interval`
    /// between ticks ; the tick itself fires every action in order.
    pub fn run(&mut self) {
        while !self.stop_flag.load(Ordering::Relaxed) {
            let started = Instant::now();
            self.tick_once();
            let elapsed = started.elapsed();
            if elapsed < self.interval {
                std::thread::sleep(self.interval - elapsed);
            }
        }
    }

    /// Run for a fixed number of ticks. Used by tests that want
    /// deterministic loop progression without spawning a stop thread.
    pub fn run_ticks(&mut self, ticks: u64) {
        for _ in 0..ticks {
            if self.stop_flag.load(Ordering::Relaxed) { break; }
            self.tick_once();
        }
    }

    /// One tick : fire every registered action, swallowing errors so
    /// a single bad dispatch can't kill the daemon. Errors print to
    /// stderr ; the audit trail for production diagnosis is heki +
    /// the existing dispatch-context breadcrumbs.
    ///
    /// First tick only : drain `bootstraps` first so the predicate-
    /// gated synthetic emissions (WokenUp on attentive-restart, i223)
    /// land BEFORE regular cadence actions on the same tick. Order
    /// matters : Mind PM must see WokenUp before BodyPulse so the
    /// instance exists when BodyPulse arrives and the wake handler
    /// can match the engaged_in_wake transition.
    pub fn tick_once(&mut self) {
        self.tick_count = self.tick_count.wrapping_add(1);
        // Freshness sweep — reload any repo whose heki file has been
        // written by a sibling process since our last touch. The
        // kernel-floor implementation of the RefreshOnPulse policy
        // declared in runtime/storage/storage.bluebook. Runs BEFORE
        // bootstraps + actions so the predicate-and-emit + cascade
        // dispatches see the freshest state. Cost when nothing
        // changed : one stat() per repo per tick. Closes the i517
        // cross-process staleness root cause.
        self.runtime.refresh_repositories_from_heki();
        if !self.bootstraps.is_empty() {
            let drained: Vec<BootstrapEmit> = std::mem::take(&mut self.bootstraps);
            for boot in drained {
                self.fire_bootstrap(boot);
            }
        }
        for action in self.actions.clone() {
            match action {
                TickAction::Emit { event_name, aggregate_type, aggregate_id, data } => {
                    let event = Event {
                        name: event_name,
                        aggregate_type,
                        aggregate_id,
                        data,
                        realm_path: None,
                        ..Default::default()
                    };
                    self.runtime.publish_synthetic_event(event);
                }
                TickAction::Dispatch { command_name, attrs } => {
                    // Resolve `{now}` / `{now+N}` / `{now-N}` clock tokens
                    // FRESH on every tick, so a cadence line like
                    // `storehouse loop ... Worker.Heartbeat stale_after={now+N}`
                    // writes the real wall-clock instant each beat (not a
                    // value frozen at registration time). HECKS_NOW still
                    // freezes it for deterministic tests.
                    let attrs = resolve_now_attrs(attrs);
                    // C3 CUTOVER — the daemon tick is ASYNC too. Deferred core
                    // mutation + ports ; domain reactions go to the outbox.
                    if let Err(e) = self.runtime.dispatch_deferred(&command_name, attrs) {
                        eprintln!("[loop_driver] dispatch error '{}': {:?}",
                                  command_name, e);
                    }
                    // Drain the outbox (persistent + in-memory fallback), then
                    // RESET the cycle guard so the next tick starts fresh — one
                    // Runtime lives across every tick, so in_flight must not leak
                    // between ticks or policies would fire only on tick 1.
                    self.runtime.pump_outbox();
                    self.runtime.pump();
                    // i750 — second drain arm : detach-spawn out-of-process
                    // adapter handlers for any OutboundEvent this tick recorded
                    // (and re-pump pending ones across ticks — free retry).
                    #[cfg(not(target_arch = "wasm32"))]
                    self.runtime.pump_outbound_events();
                    self.runtime.policy_engine.reset_in_flight();
                }
            }
        }
        // Sprint 14 — fire every attached `driving on cron` adapter
        // handler once per tick. v1 doesn't evaluate the cron
        // expression : every tick fires every cron handler. A follow-up
        // card adds expression-aware scheduling (parse 5-field cron,
        // keep last-fire-at per handler, fire only when due). No-op
        // when no hecksagons are attached or no cron handlers declared,
        // so this stays free for the historical `storehouse loop` path.
        self.runtime.fire_driving_cron_ticks();
    }

    /// i223 — evaluate a bootstrap's predicate against the current
    /// runtime state and fire its embedded emit when matched. Reads
    /// the named aggregate's identified record (looking up by
    /// aggregate_id) and compares `field` against `expected` as a
    /// string. Mismatch / missing aggregate / missing field all skip
    /// the emission silently — the bootstrap's whole point is to be
    /// inert when the predicate doesn't apply.
    ///
    /// String comparison only : the runtime stores Values in mixed
    /// shapes (Str, Int, Bool) but the bootstrap CLI surface only
    /// passes string expected values, so we render the field with
    /// `format_value_string` and compare bytes. Equivalent to how
    /// `--gate` (i108) compares heki field values.
    fn fire_bootstrap(&mut self, boot: BootstrapEmit) {
        let matched = match self.runtime.find(&boot.aggregate_type, &boot.aggregate_id) {
            Some(state) => {
                let actual = format_value_string(state.get(&boot.field));
                actual == boot.expected
            }
            None => false,
        };
        if !matched {
            eprintln!(
                "[loop_driver] bootstrap skipped : {}.{}={} did not match (aggregate {} not found or field absent)",
                boot.aggregate_type, boot.field, boot.expected, boot.aggregate_id
            );
            return;
        }
        if let TickAction::Emit { event_name, aggregate_type, aggregate_id, data } = boot.event {
            eprintln!(
                "[loop_driver] bootstrap fired : {}.{}={} → emit {}:{}:{}",
                boot.aggregate_type, boot.field, boot.expected,
                event_name, aggregate_type, aggregate_id
            );
            let event = Event {
                name: event_name,
                aggregate_type,
                aggregate_id,
                data,
                realm_path: None,
                ..Default::default()
            };
            self.runtime.publish_synthetic_event(event);
        }
    }
}

/// Render a Value into its bare string form for predicate
/// comparison. Matches the stringification the heki/cli layer
/// produces ; numeric values keep their literal digits, booleans
/// become "true"/"false", null renders empty.
fn format_value_string(v: &Value) -> String {
    match v {
        Value::Str(s) => s.clone(),
        Value::Int(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        Value::List(_) | Value::Map(_) => format!("{:?}", v),
    }
}

