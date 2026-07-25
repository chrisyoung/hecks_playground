//! canonical_naming — the canonical-address surface of dispatch : rebuild
//! the full `Realm::…::Domain::Aggregate.verb` for the LOG (canonical_for_log,
//! never inventing an address it can't build), and canonicalise a dispatch
//! name into its action/resource forms for the authz gate (CanonicalAction,
//! canonical_action, canon_forms) with the query-side aggregate lookup
//! (resolve_query_agg) and dedup_nonempty support. Consumers :
//! dispatch_core (log line) and policy_eval (authz gate).
//!
//! Cask extracted VERBATIM from runtime/command_dispatch.rs (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/canonical_naming.rs — kernel-floor
//!  dispatch naming surface, relocated verbatim from command_dispatch.rs
//!  blanket.]

use super::command_dispatch::{cmd_for, domain_matches, resolve};
use super::fqn_address::parse_fqn;
use super::Runtime;

/// The canonical FQN form of a dispatch address, for the LOG surfaces.
/// Resolves `command_name` to its aggregate and rebuilds the full
/// `Realm::…::Domain::Aggregate.verb` from the aggregate's stamped
/// `realm_path`, so a `storehouse follow` line shows WHERE the dispatch
/// actually landed — not the terse 2-seg the caller typed. A future
/// mis-resolution becomes visible in the log instead of hiding behind the
/// short form. Falls back to the raw name for legacy dotted forms,
/// unstamped aggregates (string-parsed / outside ~/Projects), or anything
/// that doesn't resolve — the log never invents an address it can't build.
pub fn canonical_for_log(rt: &Runtime, command_name: &str) -> String {
    let Some((_head, tail)) = command_name.rsplit_once('.') else {
        return command_name.to_string();
    };
    let (domain, target, _cmd) = match parse_fqn(command_name) {
        Ok(parts) => parts,
        Err(_) => return command_name.to_string(),
    };
    let domain_lc = domain.to_lowercase();
    let (realm, context) = crate::heki::fqn_realm_context(command_name);
    for (ai, agg) in rt.domain.aggregates.iter().enumerate() {
        if agg.name != target { continue; }
        if !domain_matches(rt, ai, &domain, &domain_lc) { continue; }
        if !crate::heki::realm_context_matches(agg.realm_path.as_deref(), realm.as_deref(), context.as_deref()) { continue; }
        if let Some(prefix) = crate::fqns_resolve::canonical_prefix(agg) {
            return format!("{}.{}", prefix, tail);
        }
    }
    command_name.to_string()
}

/// The canonical strings an authorization gate matches a policy against —
/// derived from the RESOLVED target, NEVER the caller's raw input, so a BARE
/// dispatch (`PlaceOrder`) and a fully-qualified one (`Pizzas::Order.PlaceOrder`)
/// of the SAME command yield IDENTICAL forms and therefore the IDENTICAL gate
/// verdict. This is the forbid-bypass fix (SECURITY-gate-matches-raw-not-fqn,
/// 2026-07-03) : before it, `evaluate_policy` matched the policy `action`
/// pattern against whatever the caller typed, so a namespace-scoped
/// `forbid Pizzas::Order.*` was escaped by dispatching the bare verb (which
/// never prefix-matched the pattern) and a namespace `permit Pizzas::*` wrongly
/// failed a bare-verb caller.
/// [antibody-exempt: rust/src/runtime/command_dispatch.rs (canonical_action) —
///  kernel-floor authz gate, security fix, authorized by Chris 2026-07-03]
///
/// `action_forms` is matched against the policy `action` ; `resource_forms`
/// against `resource`. A form SET (not a single string) keeps BOTH legacy
/// authoring conventions matching after canonicalisation — a bare-verb policy
/// (`Open`, `List`) AND a qualified one (`AuthzDemo::Vault.list`) — while every
/// form is resolution-derived, so parity across dispatch shapes holds. An
/// unresolvable phrase (the synthetic `<Aggregate>.state` reads, or genuine
/// junk) falls back to the raw string : nothing that fails to resolve can
/// dispatch anyway, so the raw fallback can never reopen the bypass, and
/// deny-by-default still bites an unknown verb.
pub(crate) struct CanonicalAction {
    pub action_forms: Vec<String>,
    pub resource_forms: Vec<String>,
    /// The RESOLVED command's declared actor (`role "System"`). `Some` only
    /// on the command branch (query / raw fallback leave it `None`, so an
    /// unresolvable phrase gets no implicit permit — fail-closed). The authz
    /// gate reads it as an implicit permit for a caller of that role
    /// (evaluate_policy), bound to the command whose forms are matched.
    pub declared_role: Option<String>,
}

pub(crate) fn canonical_action(rt: &Runtime, name: &str) -> CanonicalAction {
    // Command first — the dispatch path's OWN resolver, so the gate keys on
    // exactly what would execute.
    if let Ok(res) = resolve(rt, name, None) {
        let agg = &rt.domain.aggregates[res.agg_idx()];
        let cmd = cmd_for(rt, res);
        let verb = cmd.name.clone();
        let role = cmd.role.clone();
        let mut ca = canon_forms(agg, &verb);
        ca.declared_role = role; // the EXECUTING command's role — same resolution
        return ca;
    }
    // Then a query — by exact or snake_case verb, refusing on a homonym (an
    // ambiguous verb keeps the raw fallback rather than guess an aggregate).
    if let Some((agg, verb)) = resolve_query_agg(rt, name) {
        return canon_forms(agg, &verb);
    }
    // Unresolvable — keep the raw phrase and its tail so synthetic reads
    // (`Vault.state`) and any bare form still match a policy authored on them.
    // Resource mirrors the pre-fix derivation exactly : the head before the
    // last `.`, or the whole name when bare — never empty, so a `resource: *`
    // policy still matches (the `explain` dry path resolves no domain).
    let verb = name.rsplit('.').next().unwrap_or(name).to_string();
    let resource = name.rsplit_once('.').map(|(h, _)| h).unwrap_or(name);
    CanonicalAction {
        action_forms: dedup_nonempty(vec![name.to_string(), verb]),
        resource_forms: dedup_nonempty(vec![resource.to_string()]),
        declared_role: None, // unresolvable phrase -> no implicit permit (fail-closed)
    }
}



/// Build the action/resource form set for a RESOLVED (aggregate, verb). The
/// `Domain` segment is the aggregate's bluebook context (falling back to its
/// category), mirroring `Domain::Aggregate.verb` — the shape a policy is
/// authored against and the one `parse_fqn` reads back.
fn canon_forms(agg: &crate::ir::Aggregate, verb: &str) -> CanonicalAction {
    let verb_snake = crate::util::snake_case(verb);
    let domain = agg
        .context
        .clone()
        .or_else(|| agg.category.clone())
        .unwrap_or_default();
    let mut action_forms = Vec::new();
    let mut resource_forms = vec![agg.name.clone()];
    if domain.is_empty() {
        // No context to anchor a 2-seg FQN — expose the verb forms alone.
        action_forms.push(verb.to_string());
        action_forms.push(verb_snake);
    } else {
        let resource_fqn = format!("{}::{}", domain, agg.name);
        action_forms.push(format!("{}.{}", resource_fqn, verb));
        action_forms.push(format!("{}.{}", resource_fqn, verb_snake));
        action_forms.push(verb.to_string());
        action_forms.push(verb_snake);
        resource_forms.push(resource_fqn);
    }
    CanonicalAction {
        action_forms: dedup_nonempty(action_forms),
        resource_forms: dedup_nonempty(resource_forms),
        declared_role: None, // caller (canonical_action's command branch) overrides
    }
}

/// Find the single aggregate declaring `name`'s verb as a query — exact or
/// snake_case. `name` may be bare (`List`) or qualified (`AuthzDemo::Vault.list`) ;
/// when qualified the intended aggregate name disambiguates a homonym. Returns
/// None on no match OR ambiguity (the caller keeps the raw fallback).
fn resolve_query_agg<'a>(
    rt: &'a Runtime,
    name: &str,
) -> Option<(&'a crate::ir::Aggregate, String)> {
    let verb = name.rsplit('.').next().unwrap_or(name);
    let want_agg = if name.contains('.') {
        parse_fqn(name).ok().map(|(_d, target, _c)| target)
    } else {
        None
    };
    let mut hit: Option<(&crate::ir::Aggregate, String)> = None;
    for agg in rt.domain.aggregates.iter() {
        if let Some(ref want) = want_agg {
            if &agg.name != want {
                continue;
            }
        }
        for q in agg.queries.iter() {
            if q.name == verb || crate::util::snake_case(&q.name) == verb {
                if hit.is_some() {
                    return None; // homonym — refuse, keep raw fallback
                }
                hit = Some((agg, q.name.clone()));
            }
        }
    }
    hit
}

fn dedup_nonempty(v: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    v.into_iter()
        .filter(|s| !s.is_empty())
        .filter(|s| seen.insert(s.clone()))
        .collect()
}
