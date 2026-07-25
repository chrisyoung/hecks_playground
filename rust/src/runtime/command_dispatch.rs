//! Command dispatch — the core execution loop
//!
//! Resolves a command name to its aggregate + definition,
//! enforces givens, checks lifecycle, applies mutations,
//! transitions state, persists, and emits.
//!
//! Usage:
//!   let result = dispatch(&mut runtime, "CreatePizza", attrs)?;
//!
//! [antibody-exempt: rust/src/runtime/command_dispatch.rs — kernel-floor
//!  dispatch path. i156 added strict bare-name resolution gated by the
//!  HECKS_STRICT_DISPATCH env var ; the rest of the file is pre-i156.]

use super::{AggregateState, Event, Runtime, RuntimeError, Value};
use super::bulk_dispatch::{bulk_spec_attr, dispatch_bulk};
use super::fqn_address::realm_over_qualified_tail;
use super::fqn_resolution::resolve_fully_qualified;
use super::dispatch_diagnostics::{
    unknown_aggregate_message, unknown_command_message_2part, unknown_command_message_3part,
    unknown_command_message_bare,
};
use super::interpreter;
use super::lifecycle_defaults::{
    apply_defaults, apply_lifecycle_default, apply_lifecycle_transition, build_event_res,
    check_lifecycle, find_self_ref_res, parse_default,
};
use crate::ir::{Command, Lifecycle};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct CommandResult {
    pub aggregate_id: String,
    pub aggregate_type: String,
    pub event: Option<Event>,
    /// Event-sourcing deltas (i-event-sourcing) — the fields this command
    /// actually changed, computed as a before/after diff of aggregate state
    /// (so a lifecycle transition is just another changed field, no special-
    /// casing). `record_event_append` appends one immutable Event to the Log
    /// per delta. Empty for synthetic events and bulk-form dispatch (v1).
    pub deltas: Vec<(String, Value)>,
}

/// Resolution of a dispatch address — i111-J.
///
/// `Aggregate(agg_idx, cmd_idx)` — command lives directly on the
/// aggregate's commands list.
///
/// `Entity(agg_idx, ent_idx, cmd_idx)` — command lives inside an
/// entity declared inside the aggregate's `entity "Foo" do … end`
/// block. Dispatch still operates on the parent aggregate's record
/// (entities are reached only through the root in DDD), but uses the
/// entity's command — its mutations, givens, lifecycle.
#[derive(Debug, Clone, Copy)]
pub(super) enum Resolution {
    Aggregate(usize, usize),
    Entity(usize, usize, usize),
}

impl Resolution {
    pub(super) fn agg_idx(&self) -> usize {
        match self {
            Resolution::Aggregate(a, _) => *a,
            Resolution::Entity(a, _, _) => *a,
        }
    }
}

/// Borrow the resolved command from the runtime's IR.
pub(super) fn cmd_for(rt: &Runtime, res: Resolution) -> &Command {
    match res {
        Resolution::Aggregate(a, c) => &rt.domain.aggregates[a].commands[c],
        Resolution::Entity(a, e, c) => &rt.domain.aggregates[a].entities[e].commands[c],
    }
}

/// Borrow the lifecycle that gates the resolved command : the entity's
/// own lifecycle when present, otherwise the parent aggregate's.
pub(super) fn lifecycle_for(rt: &Runtime, res: Resolution) -> Option<&Lifecycle> {
    match res {
        Resolution::Aggregate(a, _) => rt.domain.aggregates[a].lifecycle.as_ref(),
        Resolution::Entity(a, e, _) => rt.domain.aggregates[a].entities[e]
            .lifecycle
            .as_ref()
            .or(rt.domain.aggregates[a].lifecycle.as_ref()),
    }
}
pub fn dispatch(
    rt: &mut Runtime,
    command_name: &str,
    attrs: HashMap<String, Value>,
) -> Result<CommandResult, RuntimeError> {
    dispatch_inner(rt, command_name, attrs, None)
}

/// Cascade-aware dispatch (i111-K) — used by `Runtime::drain_policies`
/// when the triggered command's aggregate has the SAME type as the
/// upstream event. Passing the upstream id as a hint lets the second
/// dispatch reuse the existing record instead of counter-minting a
/// fresh id.
///
/// This closes the i111-C surprise : multi-step pipelines like
/// CorpusPruning (Measure → Split with `given { measured == true }`)
/// previously needed `identified_by` declared just to keep the cascade
/// landing on the same row. With this hint, the cascade preserves the
/// id automatically — `identified_by` becomes a query / addressability
/// concern, not a cascade-correctness requirement.
///
/// The hint is honored only when :
///   - The triggered command's aggregate type matches `upstream_type`
///   - A record already exists at `upstream_id` in that repo
/// Otherwise the dispatch falls through to standard id resolution
/// (identified_by lookup → counter-mint), keeping cross-type cascades
/// and missing-record cases unchanged.
pub fn dispatch_cascade(
    rt: &mut Runtime,
    command_name: &str,
    attrs: HashMap<String, Value>,
    upstream_type: &str,
    upstream_id: &str,
) -> Result<CommandResult, RuntimeError> {
    dispatch_inner(rt, command_name, attrs, Some((upstream_type.to_string(), upstream_id.to_string())))
}

fn dispatch_inner(
    rt: &mut Runtime,
    command_name: &str,
    attrs: HashMap<String, Value>,
    cascade_hint: Option<(String, String)>,
) -> Result<CommandResult, RuntimeError> {
    // Caller-scoped resolution (i-fqn local-short) : the CALLER's bluebook is
    // the upstream aggregate's realm_path (cascade_hint = the aggregate whose
    // event triggered this dispatch). A SHORT ref then resolves LOCAL-first —
    // a same-name aggregate in the caller's own bluebook wins over a foreign
    // homonym. None for a top-level dispatch (no cascade), so those fall back
    // to global match and should carry a full FQN. ADDITIVE : the fallback
    // preserves today's global first-match whenever no local candidate exists.
    let caller_scope: Option<String> = cascade_hint.as_ref().and_then(|(up_type, _)| {
        rt.domain.aggregates.iter()
            .find(|a| &a.name == up_type)
            .and_then(|a| a.realm_path.clone())
    });
    let res = resolve(rt, command_name, caller_scope.as_deref())?;
    let agg_idx = res.agg_idx();

    // Many-form dispatch (i113 / i116) — only applies to aggregate-level
    // commands. Entity commands route through the parent's record and
    // don't currently support the list_of(VO) bulk pattern.
    if let Resolution::Aggregate(_, cmd_idx) = res {
        if let Some(spec_attr) = bulk_spec_attr(rt, agg_idx, cmd_idx) {
            return dispatch_bulk(rt, agg_idx, cmd_idx, command_name, &spec_attr, attrs);
        }
    }

    // create-vs-update is now a persistence upsert keyed on identity (see
    // the resolution block below) — the #729 name-heuristic and the
    // first-class-factory `is_create` bit both retired. `self_ref` survives
    // only to resolve the id a legacy `reference_to` kwarg carries.
    // PAYLOAD GATE (ACL) — value-object invariants judge the incoming
    // attrs BEFORE any hydration, id resolution, or mutation. A refusal
    // here touches nothing : no record, no event, no cascade. The domain
    // below this line can assume its payloads are valid (2026-07-18,
    // inbox/PLAN-payload-gate-acl.md).
    super::payload_gate::check(
        &rt.domain.aggregates[agg_idx],
        cmd_for(rt, res),
        &attrs,
        cascade_hint.is_some(),
    )?;

    let self_ref = find_self_ref_res(rt, res);
    let aggregate_name = rt.domain.aggregates[agg_idx].name.clone();
    let aggregate_context = rt.domain.aggregates[agg_idx].context.clone();
    let repo_hash_key = super::repo_key(aggregate_context.as_deref(), &aggregate_name);

    // ROOT-3 — singleton identity from the declared default. A singleton declares
    // `identified_by :name` + `attribute :name, default: "X"`. id_for_command keys
    // off `attrs`, but the attribute default was only applied to STATE (AFTER id
    // resolution), so a policy/PM dispatch that omitted the key counter-minted a
    // new row instead of keying the singleton (statusline 3881 rows, awareness "1",
    // the Seed gather). Inject the identity attribute's declared default into attrs
    // BEFORE id resolution, so a singleton keys by its name whether or not the
    // caller passed it. Command default wins ; else the aggregate's. No-op when the
    // key is already supplied or carries no default (non-singleton aggregates).
    let mut attrs = attrs;
    if let Some(key) = rt.domain.aggregates[agg_idx].identified_by.clone() {
        if !attrs.contains_key(&key) {
            let cmd = cmd_for(rt, res);
            let from_cmd = cmd.attributes.iter().find(|a| a.name == key);
            let from_agg = rt.domain.aggregates[agg_idx].attributes.iter().find(|a| a.name == key);
            if let Some(d) = from_cmd.and_then(|a| a.default.clone())
                .or_else(|| from_agg.and_then(|a| a.default.clone()))
            {
                let attr_type = from_cmd.map(|a| a.attr_type.clone())
                    .or_else(|| from_agg.map(|a| a.attr_type.clone()))
                    .unwrap_or_default();
                attrs.insert(key.clone(), parse_default(&d, &attr_type));
            }
        }
    }
    let attrs = attrs;

    // i111-K — same-type cascade id preservation. When the cascade
    // hopped Aggregate → Aggregate (same type) and a record exists at
    // the upstream id, reuse it. This runs BEFORE id_for_command so it
    // overrides counter-mint behavior for aggregates without
    // identified_by — closing the i111-C identified_by-required gap.
    let cascade_id = cascade_hint.as_ref().and_then(|(up_type, up_id)| {
        if up_type == &aggregate_name {
            // A same-type cascade must NOT hijack the identity of a command that
            // supplies its OWN id via the aggregate's identified_by field in attrs
            // (e.g. Claim.Acquire(story=s2) cascaded from ClaimReleased(s1) must
            // CREATE s2, not update s1). Only reuse the upstream id when the
            // command brings no explicit identity.
            let supplies_own_id = rt.domain.aggregates[agg_idx].identified_by
                .as_ref()
                .is_some_and(|idf| attrs.get(idf).is_some_and(|v| !v.to_string().is_empty()));
            if supplies_own_id {
                return None;
            }
            rt.repositories.get(&repo_hash_key)
                .and_then(|repo| repo.find(up_id))
                .map(|_| up_id.clone())
        } else {
            None
        }
    });

    // Build the diagnostic up front — borrowing `rt` inside
    // `ok_or_else` collides with the `get_mut` mutable borrow.
    let unknown_agg_msg = if rt.repositories.contains_key(&repo_hash_key) {
        String::new()
    } else {
        unknown_aggregate_message(rt, &aggregate_name)
    };
    // i-cascade-fk : a cross-aggregate cascade (cascade_hint upstream type
    // differs from this aggregate) driving a self-ref command carries the
    // upstream event's leaked `id`, which names the UPSTREAM record, not one
    // of this type. Resolve the owning record from the upstream record's FK
    // (its attribute whose declared type IS this aggregate) so e.g.
    // TaskCompleted -> Story.DropPendingTaskCount finds the Story via Task.story.
    let cascade_fk_id: Option<String> = cascade_hint.as_ref().and_then(|(up_type, up_id)| {
        if up_type == &aggregate_name { return None; }
        // same-context upstream first : in combined-domain mode several
        // domains may share an aggregate name (e.g. Task), so prefer the one
        // in the target's context before falling back to any match.
        let up_agg = rt.domain.aggregates.iter()
            .find(|a| &a.name == up_type && a.context == aggregate_context)
            .or_else(|| rt.domain.aggregates.iter().find(|a| &a.name == up_type))?;
        let fname = up_agg.attributes.iter()
            .find(|at| at.attr_type == aggregate_name)
            .map(|at| at.name.clone())?;
        rt.repositories.get(&super::repo_key(up_agg.context.as_deref(), up_type))
            .and_then(|up| up.find(up_id))
            .and_then(|rec| rec.fields.get(&fname))
            .and_then(|v| v.as_str().map(str::to_string))
    });
    // i735 defect 2 — an aggregate whose `:sqlite` adapter was refused at
    // boot has NO repository (dropped, never silently swapped to heki).
    // Surface the loud reason instead of a misleading "unknown aggregate".
    if let Some(reason) = rt.refused_persistence.get(&repo_hash_key) {
        return Err(RuntimeError::PersistenceRefused(reason.clone()));
    }
    let repo = rt.repositories.get_mut(&repo_hash_key)
        .ok_or(RuntimeError::UnknownAggregate(unknown_agg_msg))?;

    // Pure upsert on identity (Relationship grammar, 2026-06-18) — create
    // vs update is a PERSISTENCE contract, not a declaration. Resolve the
    // identity from every source, then find-or-create : present -> update,
    // absent -> insert. No name heuristic, no reference_to(Self) error-on-
    // absent — the row's existence IS the create/update bit, so AddStory
    // can no longer mint a phantom Sprint. `find_self_ref_res` survives
    // only to keep resolving the id a legacy `reference_to` kwarg carries
    // (e.g. `sprint=`) until the corpus drops the self-ref form ; new
    // commands resolve via the universal `id` key or id_for_command.
    let resolved_id = if let Some(ref_name) = &self_ref {
        attrs.get(ref_name).map(|v| v.to_string())
            .or_else(|| cascade_fk_id.clone())
            .or_else(|| attrs.get("id").map(|v| v.to_string()))
            .or_else(|| cascade_id.clone())
            .unwrap_or_else(|| repo.id_for_command(&attrs))
    } else if let Some(id) = cascade_id.clone() {
        id
    } else {
        repo.id_for_command(&attrs)
    };
    let (mut state, is_new) = match repo.find(&resolved_id).cloned() {
        Some(s) => (s, false),
        None => (AggregateState::new(&resolved_id), true),
    };

    // Event-sourcing delta capture (i-event-sourcing) — snapshot state
    // BEFORE the command's effects (defaults, mutations, transition). For a
    // new aggregate this is empty, so every resolved field (including the
    // applied defaults and the lifecycle transition) surfaces as a delta in
    // the before/after diff computed just before save.
    let before_fields = state.fields.clone();

    if is_new {
        apply_defaults(rt, agg_idx, &mut state);
        apply_lifecycle_default(rt, agg_idx, &mut state);
    }

    // Create commands copy matching attrs to aggregate. For entity
    // commands the parent aggregate's attribute set is the target
    // schema (entities live within the parent's record). This phase runs
    // BEFORE pipeline_core (givens -> mutations -> invariants), so a
    // pure-data create like EventSourcing::Event.Append populates its VO
    // attrs here and check_invariants enforces on the REAL values, not
    // the apply_defaults empty-list placeholders. then_set mutations run
    // after and override on overlap (no corpus then_set targets a raw
    // create-attr name, so this is inert for existing aggregates).
    if is_new {
        let agg_attr_names: Vec<&str> = rt.domain.aggregates[agg_idx]
            .attributes.iter().map(|a| a.name.as_str()).collect();
        let cmd = cmd_for(rt, res);
        for cmd_attr in &cmd.attributes {
            if agg_attr_names.contains(&cmd_attr.name.as_str()) {
                if let Some(val) = attrs.get(&cmd_attr.name) {
                    state.set(&cmd_attr.name, val.clone());
                } else if let Some(ref d) = cmd_attr.default {
                    // The COMMAND's declared default, when the caller
                    // omitted the attribute. Previously only a supplied
                    // value ever reached state, so
                    // `attribute :currency, Currency, default: "USD"` on a
                    // command was inert — the field simply stayed unset,
                    // and the declaration said something the runtime did
                    // not do. Aggregate-level defaults already applied
                    // (apply_defaults) ; this closes the command-level half.
                    state.set(&cmd_attr.name, parse_default(d, &cmd_attr.attr_type));
                }
            }
        }
        // Store belongs_to FKs supplied as create inputs, symmetric with
        // the belongs_to-on-event emit below : a create command's
        // `reference_to X` input names an aggregate belongs_to (`tool`,
        // `member`), but references are not attributes, so the loop above
        // dropped them. Without this the stored FK the event-emit reads is
        // always empty, and a downstream policy needing that FK falls to
        // the singleton fallback (arbitrary record). Completes the
        // half-wired belongs_to-on-event feature. VALIDATOR-cross-
        // aggregate-policy-ref-naming.
        // LegacyReferenceTo (`reference_to X`) is treated as BelongsTo for IR
        // shape (ir.rs), so treat it as BelongsTo for PERSISTENCE too : a
        // declared relationship is stored under its bare name, no hand-written
        // `then_set :x_id`. This is what lets a domain speak relationships
        // instead of ids (banking's customer/source/destination/account).
        for r in &rt.domain.aggregates[agg_idx].references {
            if matches!(
                r.kind,
                crate::ir::ReferenceKind::BelongsTo
                    | crate::ir::ReferenceKind::LegacyReferenceTo
            ) {
                if let Some(val) = attrs.get(&r.name) {
                    state.set(&r.name, val.clone());
                }
            }
        }
    }

    // Pipeline: givens → lifecycle check → mutations → lifecycle transition
    let cmd = cmd_for(rt, res);
    // The type context a given resolves against : the aggregate's value
    // objects + a name→VO-type map, so `given { balance.covers?(amount) }`
    // can find the `covers?` derivation on Money. Built per dispatch ; only
    // production givens carry it (invariants / payload gate pass empty).
    let given_attr_types = interpreter::build_attr_types(cmd, &rt.domain.aggregates[agg_idx]);
    let given_ctx = interpreter::EvalCtx::for_command(
        &rt.domain.aggregates[agg_idx].value_objects,
        &given_attr_types,
    );
    interpreter::check_givens(cmd, &state, &attrs, given_ctx)?;
    check_lifecycle(rt, res, &state)?;
    interpreter::apply_mutations(cmd, &mut state, &attrs);
    apply_lifecycle_transition(rt, res, &mut state);

    // f4 — aggregate-level invariants are checked on the RESULTING state
    // (post-mutation, post-transition) before save. A violated invariant
    // rejects the command exactly like a failed `given` : 0 events, no save.
    // An invariant is a `given` enforced on every command.
    if let Some(violated) = crate::invariants::check_invariants(
        &rt.domain.aggregates[agg_idx].invariants,
        &state,
        &attrs,
    ) {
        return Err(RuntimeError::InvariantViolation {
            name: violated.name.clone(),
            expression: violated.expression.clone(),
        });
    }

    // belongs_to-on-event — the aggregate's stored belongs_to FKs ride its
    // emitted event so downstream adapters can bind them (the freed worker on
    // ClaimReleased -> SEAM 1 retrigger ; the story on LeaseReclaimed -> worktree
    // unlock). Command inputs win on collision ; empty FKs are skipped.
    let mut event_data = attrs.clone();
    for r in &rt.domain.aggregates[agg_idx].references {
        if matches!(
            r.kind,
            crate::ir::ReferenceKind::BelongsTo
                | crate::ir::ReferenceKind::LegacyReferenceTo
        ) && !event_data.contains_key(&r.name)
        {
            let v = state.get(&r.name);
            if !v.to_string().is_empty() {
                event_data.insert(r.name.clone(), v.clone());
            }
        }
    }
    // Event-sourcing deltas — the fields whose value changed vs. the
    // pre-command snapshot. A lifecycle transition is just another changed
    // field here (status: pending -> authorized), so it needs no special-
    // casing. Computed BEFORE save (which moves state). record_event_append
    // appends one immutable Log Event per delta.
    let deltas: Vec<(String, Value)> = state.fields.iter()
        .filter(|(k, v)| before_fields.get(*k) != Some(*v))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    // deciderate Layer 0b — the CHANGED state fields (deltas) ride the emitted
    // event so a policy can GUARD on the RESULTING state, not just the command
    // inputs (e.g. `where games_remaining: 0` after a decrement — the Bracket
    // saga's round-complete condition). The delta WINS over a same-named
    // command input : the event must carry what the command RESULTED IN, not
    // what was requested. This is essential when a for_each SWEEP passes the
    // swept record's PRE-mutation field (e.g. Bubble.Decay receives the floating
    // bubble's old `size`) and then mutates it — the guard (`where size: 0`)
    // must see the post-decrement value, not the stale swept input. Untouched
    // input fields (never in deltas) are left as-is.
    for (k, v) in &deltas {
        event_data.insert(k.clone(), v.clone());
    }
    let aggregate_id = state.id.clone();
    let was_deleted = state.deleted;
    let ctx = crate::heki::WriteContext::Dispatch {
        aggregate: &aggregate_name, command: command_name,
    };
    let repo = rt.repositories.get_mut(&repo_hash_key).unwrap();
    if was_deleted {
        repo.delete(&aggregate_id, ctx);
    } else {
        repo.save(state, ctx);
    }

    let mut event = build_event_res(rt, res, &aggregate_id, &event_data);
        if let Some(ref mut evt) = event {
            // Causation-tree view — stamp this event's parent link before publish.
            // A cascaded dispatch cites the event that triggered it : the last
            // event the upstream (cascade_hint) aggregate published. None for a
            // root dispatch, which leaves this event a tree root. Gate-independent
            // of the durable Log — reads the in-memory bus directly.
            if let Some((up_type, up_id)) = &cascade_hint {
                evt.causation_id = rt.event_bus.last_event_for(up_type, up_id);
            }
            // Sprint 14 (retire-sync-cascade-pipeline) — the legacy Sync vs.
            // Actor fork retired. Every aggregate publishes inline through the
            // event bus ; the `delivery :actor` mailbox-stub path is gone. The
            // actor-per-aggregate-instance + async-event-delivery-bus stories
            // will rewire publishing into per-mailbox enqueueing later without
            // a bluebook-visible fork — the runtime is the swappable substrate.
            rt.event_bus.publish(evt.clone());
        }

    let result = CommandResult {
        aggregate_id,
        aggregate_type: aggregate_name,
        event,
        deltas,
    };
    // Event-sourcing : append to the durable Log at the UNIVERSAL door, so
    // every command that passes through storehouse — top-level dispatch AND
    // cascade reaction (which reaches dispatch_inner via dispatch_cascade,
    // bypassing the Runtime::dispatch wrapper) — is recorded. record_event_append
    // guards against recursion (its own Append) and infra (CascadeRun/OutboundEvent).
    // Phase-4 causation : a cascaded dispatch carries its upstream (type, id) ;
    // the cause is the last Log event recorded for that aggregate. Root
    // dispatches (no hint) resolve to empty — where CausationTrace stops.
    let causation_id = rt.cause_for_cascade(&cascade_hint);
    // Per-flow correlation : mint ONE id at the ROOT of a flow (no cascade hint)
    // and let every cascade the flow triggers inherit it (cascade_hint = Some
    // skips the mint), so all events of one business flow (a transfer saga)
    // share a correlation_id. Re-minting at each root keeps flows from bleeding
    // together. The writer reads rt.current_correlation.
    if cascade_hint.is_none() {
        rt.current_correlation = Some(rt.mint_correlation_id(command_name));
    }
    // Governability : a ROOT dispatch (no cascade hint) carries the principal +
    // verdict captured at the entry gate ; take it once so it can never leak to a
    // later gate-bypassing dispatch. A cascade passes None and the writer records
    // the honest `system` actor — its originating actor rides correlation_id.
    let captured_auth = if cascade_hint.is_none() { rt.current_auth.take() } else { None };
    // COMPLETENESS — record the DEBT before the writer runs. Counted here, at the
    // caller, and never inside record_event_append : a writer cannot be its own
    // oracle. The empty-delta guard that silently swallowed event-rows was an early
    // `return`, and a counter placed after it would have been skipped just as
    // quietly. Owed here, written there, and `missing_event_rows()` is the gap.
    if let Some(ev) = &result.event {
        if rt.owes_event_row(&ev.aggregate_type) {
            rt.event_rows_owed += 1;
        }
    }
    rt.record_event_append(&result, command_name, &causation_id, captured_auth);
    // Event Log consolidation : the Consolidate maintenance command folds this
    // realm's per-process shards into the global ordered event.heki. Hooked
    // HERE (not the Runtime::dispatch wrapper) so it fires on BOTH the manual
    // dispatch path AND the driver's dispatch_cascade fire (storehouse drive).
    // No-op for every other command. Replaces the hand-written run_merge daemon.
    rt.run_consolidate_if(command_name);
    // Derivability as a STANDING invariant — the Verification trigger's sibling
    // hook. Fires only for EventSourcing::Verification.Verify, so every other
    // dispatch pays one string compare.
    rt.run_verification_if(command_name);
    Ok(result)
}

pub(super) fn resolve(rt: &Runtime, command_name: &str, caller_scope: Option<&str>) -> Result<Resolution, RuntimeError> {
        // i560 v2 — canonical FQN form first. Presence of `::` is the
        // discriminator ; the rest falls through to the legacy dotted
        // forms that internal cascade dispatch relies on.
        if command_name.contains("::") {
            match resolve_fully_qualified(rt, command_name, caller_scope) {
                Ok(r) => return Ok(r),
                Err(e) => {
                    // Realm-tolerant fallback (FQN over-qualification, 2026-06-30).
                    // A dispatch target carrying MORE namespace segments than the
                    // canonical `Domain::Aggregate.Command` tail — e.g. the
                    // hecksagon `result_into`
                    // `Hecks::Framework::Cascade::Cascade.RecordResult` — derives a
                    // realm/context from the extra segments that no aggregate's
                    // real realm_path matches, so the strict resolve filters every
                    // candidate out. Retry with the bare two-segment tail
                    // (`Cascade::Cascade.RecordResult`) before surfacing the error.
                    // The tail path keeps its own ambiguity guard (a homonym across
                    // realms still errors), so this never silently mis-resolves ;
                    // it only fires AFTER the strict form already failed, so a
                    // correct full FQN is never overridden.
                    //
                    // The retry passes caller_scope = None : an over-qualified
                    // target EXPLICITLY named its bluebook, so it is a global cross-
                    // bluebook reference, not a scope-relative short ref. Passing the
                    // caller's scope would trip the foreign-bluebook guard (a realm-
                    // omitted ref resolving outside the caller's scope is rejected),
                    // which is exactly the cascade case (Tools -> Cascade). Global
                    // resolution still errors on a genuine cross-realm homonym.
                    if let Some(tail) = realm_over_qualified_tail(command_name) {
                        if let Ok(r) = resolve_fully_qualified(rt, &tail, None) {
                            return Ok(r);
                        }
                    }
                    return Err(e);
                }
            }
        }
    let parts: Vec<&str> = command_name.split('.').collect();
    match parts.as_slice() {
        [a, b, c] => {
            // Try Context.Aggregate.Command first (i142). If no
            // aggregate is in that context, fall through to the
            // Aggregate.Entity.Command form (i111-J).
            for (ai, agg) in rt.domain.aggregates.iter().enumerate() {
                if agg.name != *b { continue; }
                if agg.context.as_deref() != Some(*a) { continue; }
                for (ci, cmd) in agg.commands.iter().enumerate() {
                    if cmd.name == *c { return Ok(Resolution::Aggregate(ai, ci)); }
                }
            }
            for (ai, agg) in rt.domain.aggregates.iter().enumerate() {
                if agg.name != *a { continue; }
                for (ei, ent) in agg.entities.iter().enumerate() {
                    if ent.name != *b { continue; }
                    for (ci, cmd) in ent.commands.iter().enumerate() {
                        if cmd.name == *c { return Ok(Resolution::Entity(ai, ei, ci)); }
                    }
                }
            }
            Err(RuntimeError::UnknownCommand(
                unknown_command_message_3part(rt, command_name, a, b, c)
            ))
        }
        [agg_name, cmd_name] => {
            // First pass — direct aggregate command match. LOCAL-first
            // (i-fqn local-short) : a same-name aggregate in the caller's own
            // bluebook wins outright. Else decide by UNIQUENESS — a unique target
            // resolves even when foreign (mirrors the lenient bare-name path every
            // cross-bluebook PM `dispatch` / policy `then trigger` short ref relies
            // on). Only a genuine HOMONYM — the same Aggregate.Command in 2+
            // DISTINCT stamped bluebooks, none local — must use the full FQN.
            // (pulse-pm fix : the prior guard errored on the FIRST foreign match
            // without counting, silently killing cross-bluebook PM dotted refs.)
            let mut matches: Vec<(Resolution, Option<String>)> = Vec::new();
            for (ai, agg) in rt.domain.aggregates.iter().enumerate() {
                if agg.name != *agg_name { continue; }
                for (ci, cmd) in agg.commands.iter().enumerate() {
                    if cmd.name == *cmd_name {
                        if caller_scope.is_some() && agg.realm_path.as_deref() == caller_scope {
                            return Ok(Resolution::Aggregate(ai, ci));
                        }
                        matches.push((Resolution::Aggregate(ai, ci), agg.realm_path.clone()));
                    }
                }
            }
            if !matches.is_empty() {
                let distinct_scopes: std::collections::HashSet<&str> =
                    matches.iter().filter_map(|(_, rp)| rp.as_deref()).collect();
                if caller_scope.is_some() && distinct_scopes.len() > 1 {
                    return Err(RuntimeError::UnknownCommand(format!(
                        "{} — short ref is ambiguous across {} bluebooks (caller scope {:?}); a cross-bluebook cascade must use the full Realm::Context::Bluebook::Aggregate FQN",
                        command_name, distinct_scopes.len(), caller_scope)));
                }
                return Ok(matches.into_iter().next().unwrap().0);
            }
            // Second pass (i111-J) — entity-owned command, accepted
            // only when unambiguous (single owning entity within the
            // aggregate). Multiple owners on the same aggregate is a
            // corpus error ; surface it loudly.
            let mut hits: Vec<(usize, usize, usize, String)> = Vec::new();
            for (ai, agg) in rt.domain.aggregates.iter().enumerate() {
                if agg.name != *agg_name { continue; }
                for (ei, ent) in agg.entities.iter().enumerate() {
                    for (ci, cmd) in ent.commands.iter().enumerate() {
                        if cmd.name == *cmd_name {
                            hits.push((ai, ei, ci, ent.name.clone()));
                        }
                    }
                }
            }
            if hits.len() == 1 {
                Ok(Resolution::Entity(hits[0].0, hits[0].1, hits[0].2))
            } else if hits.is_empty() {
                Err(RuntimeError::UnknownCommand(
                    unknown_command_message_2part(rt, command_name, agg_name, cmd_name)
                ))
            } else {
                // Multiple entities of the same aggregate own a command
                // by this name. Without entity disambiguation in the
                // address, the runtime can't pick. Same shape as i156
                // bare-name ambiguity ; reuse UnknownCommand with the
                // candidate list embedded in the message.
                let mut candidates: Vec<String> = hits.iter()
                    .map(|h| format!("{}.{}", agg_name, h.3))
                    .collect();
                candidates.sort();
                candidates.dedup();
                Err(RuntimeError::UnknownCommand(format!(
                    "{} — ambiguous, candidates: {}",
                    command_name,
                    candidates.join(", ")
                )))
            }
        }
        [cmd_name] => {
            // Bare-name dispatch — i156 strict mode (opt-in via the
            // HECKS_STRICT_DISPATCH env var). When strict mode is on AND
            // the bare name appears on more than one aggregate across
            // the loaded corpus, fail loudly with the candidate list
            // instead of silently first-match-wins.
            //
            // Strict mode stays opt-in until the corpus migration is
            // complete (the validator_corpus::bare_name_collisions rule
            // surfaces what's left ; ~107 .behaviors setups already
            // migrated to qualified form by i156 part 2). The i156 PR
            // body documents the gating decision.
            //
            // Default (non-strict) behavior preserves first-match-wins
            // for backward compat — same as before. Per-aggregate
            // uniqueness (i155) still holds within a single bluebook.
            //
            // i111-J — the walk also visits entity-owned commands so
            // collapsed-entity behaviors can still dispatch by short
            // name. Aggregate commands take precedence ; entities are
            // the fallback. Strict-mode collision counts include
            // entity owners alongside aggregate owners.
            let strict = std::env::var("HECKS_STRICT_DISPATCH")
                .map(|v| v == "1" || v == "true")
                .unwrap_or(false);
            if strict {
                let mut hits: Vec<(Resolution, String)> = Vec::new();
                for (ai, agg) in rt.domain.aggregates.iter().enumerate() {
                    for (ci, cmd) in agg.commands.iter().enumerate() {
                        if cmd.name == *cmd_name {
                            hits.push((Resolution::Aggregate(ai, ci), agg.name.clone()));
                        }
                    }
                    for (ei, ent) in agg.entities.iter().enumerate() {
                        for (ci, cmd) in ent.commands.iter().enumerate() {
                            if cmd.name == *cmd_name {
                                hits.push((
                                    Resolution::Entity(ai, ei, ci),
                                    format!("{}.{}", agg.name, ent.name),
                                ));
                            }
                        }
                    }
                }
                match hits.len() {
                    0 => Err(RuntimeError::UnknownCommand(
                        unknown_command_message_bare(rt, command_name)
                    )),
                    1 => Ok(hits[0].0),
                    _ => {
                        let mut candidates: Vec<String> = hits.iter().map(|h| h.1.clone()).collect();
                        candidates.sort();
                        candidates.dedup();
                        Err(RuntimeError::AmbiguousCommand {
                            name: cmd_name.to_string(),
                            candidates,
                        })
                    }
                }
            } else {
                for (ai, agg) in rt.domain.aggregates.iter().enumerate() {
                    for (ci, cmd) in agg.commands.iter().enumerate() {
                        if cmd.name == *cmd_name { return Ok(Resolution::Aggregate(ai, ci)); }
                    }
                }
                for (ai, agg) in rt.domain.aggregates.iter().enumerate() {
                    for (ei, ent) in agg.entities.iter().enumerate() {
                        for (ci, cmd) in ent.commands.iter().enumerate() {
                            if cmd.name == *cmd_name {
                                return Ok(Resolution::Entity(ai, ei, ci));
                            }
                        }
                    }
                }
                Err(RuntimeError::UnknownCommand(
                    unknown_command_message_bare(rt, command_name)
                ))
            }
        }
        _ => Err(RuntimeError::UnknownCommand(format!(
            "{} — malformed dispatch address (canonical form is 'Domain::Aggregate.Command' or 'Domain::Aggregate.query_name'; legacy forms still accepted for cascade: 'Command', 'Aggregate.Command', 'Context.Aggregate.Command', 'Aggregate.Entity.Command')",
            command_name
        ))),
    }
}

