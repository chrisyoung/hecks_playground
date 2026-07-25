//! pm_persistence — Phase D of the PM engine : heki persistence. Production
//! daemons fork storehouse per dispatch ; without persistence each
//! subprocess builds an empty PMEngine and transitions don't accumulate.
//! Routes each PM's instances through
//! `<data_dir>/process_managers/<pm_snake>.heki`, keyed by correlation_id,
//! last-write-wins (load_persisted at boot, persist_instance per
//! transition, pm_heki_path convention). The reactive core stays in
//! pm_engine.rs.
//!
//! Cask extracted VERBATIM from runtime/pm_engine.rs (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/pm_persistence.rs — kernel-floor PM
//!  persistence, relocated verbatim from pm_engine.rs blanket.]

use super::pm_engine::{PMEngine, PMInstanceState};
use crate::heki;
use std::collections::HashMap;

impl PMEngine {
    // ---- Phase D : heki persistence -----------------------------------
    //
    // Production daemons fork storehouse per dispatch. Without persistence,
    // each subprocess builds an empty PMEngine and transitions don't
    // accumulate across ticks. Persistence routes each PM's instances
    // through `<data_dir>/process_managers/<pm_snake>.heki`. Records are
    // keyed by correlation_id ; each record carries state + last_event.
    // Last-write-wins on race ; the daemon model dispatches one event
    // per fork so concurrent writes to the same instance are rare.

    /// Load existing PM instances from heki for every registered PM.
    /// Called once at Runtime boot after register. No-op when data_dir
    /// is None (in-memory test runtimes don't persist).
    pub fn load_persisted(&mut self, data_dir: Option<&str>) {
        let dir = match data_dir {
            Some(d) => d,
            None => return,
        };
        let names: Vec<String> = self.bindings.keys().cloned().collect();
        for pm_name in names {
            let path = pm_heki_path(dir, &pm_name);
            let store = match heki::read(&path) {
                Ok(s) => s,
                Err(_) => continue,
            };
            if let Some(binding) = self.bindings.get_mut(&pm_name) {
                for (correlation_id, record) in store {
                    let state = record
                        .get("state")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let last_event = record
                        .get("last_event")
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    // Phase 2.c — read the per-instance attributes hash
                    // back. The on-disk shape is a JSON object under
                    // the "attributes" key ; missing key (legacy
                    // pre-2.c records) loads as an empty hash.
                    let attributes = record
                        .get("attributes")
                        .and_then(|v| v.as_object())
                        .map(|obj| {
                            obj.iter()
                                .filter_map(|(k, v)| {
                                    v.as_str().map(|s| (k.clone(), s.to_string()))
                                })
                                .collect::<HashMap<String, String>>()
                        })
                        .unwrap_or_default();
                    binding.instances.insert(
                        correlation_id.clone(),
                        PMInstanceState {
                            correlation_id,
                            state,
                            last_event,
                            attributes,
                        },
                    );
                }
            }
        }
    }

    /// Persist one instance's current state to heki. Called by Runtime
    /// after each successful react. Idempotent — re-persisting the
    /// same state is a no-op heki-side besides bumping updated_at.
    pub fn persist_instance(
        &self,
        pm_name: &str,
        correlation_id: &str,
        data_dir: Option<&str>,
    ) -> Result<(), String> {
        let dir = match data_dir {
            Some(d) => d,
            None => return Ok(()),
        };
        let binding = self
            .bindings
            .get(pm_name)
            .ok_or_else(|| format!("pm_engine.persist : no binding for {}", pm_name))?;
        let inst = binding
            .instances
            .get(correlation_id)
            .ok_or_else(|| format!("pm_engine.persist : no instance for {}/{}", pm_name, correlation_id))?;

        let mut record: heki::Record = HashMap::new();
        record.insert(
            "id".to_string(),
            serde_json::Value::String(correlation_id.to_string()),
        );
        record.insert(
            "state".to_string(),
            serde_json::Value::String(inst.state.clone()),
        );
        if let Some(le) = &inst.last_event {
            record.insert(
                "last_event".to_string(),
                serde_json::Value::String(le.clone()),
            );
        }
        // Phase 2.c — round-trip per-instance attributes. Always
        // emitted (even when empty) so loaders see a consistent shape ;
        // empty hash deserialises identically to the missing-key case
        // for legacy records.
        let mut attrs_obj = serde_json::Map::new();
        for (k, v) in &inst.attributes {
            attrs_obj.insert(k.clone(), serde_json::Value::String(v.clone()));
        }
        record.insert(
            "attributes".to_string(),
            serde_json::Value::Object(attrs_obj),
        );

        let path = pm_heki_path(dir, pm_name);
        // Ensure the parent dir exists before heki tries to write.
        if let Some(parent) = std::path::Path::new(&path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        heki::upsert(
            &path,
            &record,
            heki::WriteContext::Dispatch {
                aggregate: pm_name,
                command: "PMTransition",
            },
        )?;
        Ok(())
    }
}


/// Heki path for a process manager's instance store.
/// Convention : `<data_dir>/process_managers/<pm_snake>.heki`
fn pm_heki_path(data_dir: &str, pm_name: &str) -> String {
    let snake = crate::util::snake_case(pm_name);
    let trimmed = data_dir.trim_end_matches('/');
    format!("{}/process_managers/{}.heki", trimmed, snake)
}
