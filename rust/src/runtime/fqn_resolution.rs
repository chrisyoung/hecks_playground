//! fqn_resolution — the corpus-backed half of FQN resolution : walk the
//! loaded aggregates to bind a parsed address to a Resolution
//! (resolve_fully_qualified, honouring realm/context stamps, caller scope,
//! entity commands, and the ambiguity guard) plus the domain_matches
//! predicate. The pure address grammar lives in fqn_address.rs ; the
//! legacy-form resolve loop stays in command_dispatch.rs.
//!
//! Cask extracted VERBATIM from runtime/command_dispatch.rs (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/fqn_resolution.rs — kernel-floor
//!  dispatch resolution, relocated verbatim from command_dispatch.rs
//!  blanket.]

use super::command_dispatch::Resolution;
use super::fqn_address::{ambiguity_candidates, parse_fqn};
use super::{Runtime, RuntimeError};

/// Resolve the fully-qualified canonical form
/// `Domain::Aggregate.Command`. The trailing token is the command
/// name (PascalCase) ; queries are resolved by the query layer using
/// `parse_fqn` directly, so this function only returns a Resolution
/// for command-shaped addresses.
pub(super) fn resolve_fully_qualified(rt: &Runtime, command_name: &str, caller_scope: Option<&str>) -> Result<Resolution, RuntimeError> {
    let (domain, target, cmd) = match parse_fqn(command_name) {
        Ok(parts) => parts,
        Err(msg) => return Err(RuntimeError::UnknownCommand(msg)),
    };

    let domain_lc = domain.to_lowercase();
    let (realm, context) = crate::heki::fqn_realm_context(command_name);

    // First pass — aggregate-rooted command (target == aggregate name).
    // The FLIP (i-fqn): collect EVERY realm-matching candidate rather than
    // taking the first. A realm-OMITTED (2-seg) dispatch that resolves to >1
    // aggregate across distinct realms is reported AmbiguousCommand instead of
    // silently picking the first — the Tools::Cascade wrong-realm failure mode.
    // This is a no-op while the merged corpus has zero cross-realm collisions ;
    // it only bites when a genuine homonym appears. A realm-QUALIFIED dispatch
    // (realm.is_some()) keeps the first-match behaviour — its realm already
    // disambiguated via realm_context_matches.
    let mut hits: Vec<Resolution> = Vec::new();
    let mut hit_realms: Vec<String> = Vec::new();
    for (ai, agg) in rt.domain.aggregates.iter().enumerate() {
        if agg.name != target { continue; }
        if !domain_matches(rt, ai, &domain, &domain_lc) { continue; }
        if !crate::heki::realm_context_matches(agg.realm_path.as_deref(), realm.as_deref(), context.as_deref()) { continue; }
        for (ci, c) in agg.commands.iter().enumerate() {
            if c.name == cmd {
                hits.push(Resolution::Aggregate(ai, ci));
                hit_realms.push(agg.realm_path.clone().unwrap_or_default());
            }
        }
    }
    if hits.len() == 1 {
        // TIGHTEN (i-fqn) : a scoped (cascade) realm-OMITTED short ref whose single
        // hit is a FOREIGN stamped bluebook must use the full FQN. Top-level (no
        // scope), full-FQN (realm given), and unstamped hits keep first-match.
        if caller_scope.is_some() && realm.is_none() {
            let rp = hit_realms[0].as_str();
            if !rp.is_empty() && Some(rp) != caller_scope {
                return Err(RuntimeError::UnknownCommand(format!(
                    "{} — short ref resolves only to a FOREIGN bluebook (caller scope {:?}); a cross-bluebook cascade must use the full Realm::Context::Bluebook::Aggregate FQN",
                    command_name, caller_scope)));
            }
        }
        return Ok(hits.remove(0));
    }
    if hits.len() > 1 {
        // LOCAL-first (i-fqn local-short) : when the caller's bluebook is known
        // and exactly one hit lives in it, the local aggregate wins over foreign
        // homonyms — no ambiguity error, no first-match guess.
        if let Some(scope) = caller_scope {
            let local: Vec<usize> = hit_realms.iter().enumerate()
                .filter(|(_, rp)| rp.as_str() == scope)
                .map(|(i, _)| i)
                .collect();
            if local.len() == 1 {
                return Ok(hits.remove(local[0]));
            }
            // TIGHTEN : a scoped realm-omitted cascade ref with NO unique local
            // hit spans only foreign bluebooks — error rather than first-match.
            if realm.is_none() {
                return Err(RuntimeError::UnknownCommand(format!(
                    "{} — short ref has no unique local match in caller scope {:?} and spans multiple bluebooks; use the full FQN",
                    command_name, caller_scope)));
            }
        }
        if let Some(candidates) = ambiguity_candidates(&hit_realms, realm.is_none(), command_name) {
            return Err(RuntimeError::AmbiguousCommand {
                name: command_name.to_string(),
                candidates,
            });
        }
        return Ok(hits.remove(0));
    }

    // Second pass — entity-owned command. The canonical form uses the
    // entity name in the command (e.g. `Discipline::Macrophage.CheckRegister`
    // for a Register command on the Check entity). But for parity with
    // the legacy `Aggregate.Entity.Command` form, we also accept lookups
    // where `target` is an aggregate name and the command lives on one
    // of its entities — useful for one-shot dispatches without renaming
    // the entity command.
    for (ai, agg) in rt.domain.aggregates.iter().enumerate() {
        if agg.name != target { continue; }
        if !domain_matches(rt, ai, &domain, &domain_lc) { continue; }
        if !crate::heki::realm_context_matches(agg.realm_path.as_deref(), realm.as_deref(), context.as_deref()) { continue; }
        for (ei, ent) in agg.entities.iter().enumerate() {
            for (ci, c) in ent.commands.iter().enumerate() {
                if c.name == cmd {
                    return Ok(Resolution::Entity(ai, ei, ci));
                }
            }
        }
    }

    Err(RuntimeError::UnknownCommand(format!(
        "{} — no command at this address. Checked domain '{}' for aggregate '{}' and command '{}'.",
        command_name, domain, target, cmd
    )))
}

/// Does the FQN's first segment match the aggregate's bluebook
/// context or category? Case-insensitive match — `Discipline` and
/// `discipline` both resolve to `category "discipline"`.
pub(super) fn domain_matches(rt: &Runtime, agg_idx: usize, domain: &str, domain_lc: &str) -> bool {
    let agg = &rt.domain.aggregates[agg_idx];
    if agg.context.as_deref() == Some(domain) { return true; }
    if let Some(ref ctx) = agg.context {
        if ctx.to_lowercase() == *domain_lc { return true; }
    }
    // i560 v2 — match against the aggregate's stamped category so
    // `Discipline::Macrophage.Run` resolves through the merged corpus.
    if let Some(ref cat) = agg.category {
        if cat == domain || cat.to_lowercase() == *domain_lc { return true; }
    }
    // Single-domain fallback : when the runtime was booted from a
    // single .bluebook the per-Domain category is still set.
    if let Some(ref cat) = rt.domain.category {
        if cat.to_lowercase() == *domain_lc { return true; }
    }
    false
}
