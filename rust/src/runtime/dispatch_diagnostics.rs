//! dispatch_diagnostics — unknown-name error messages for command dispatch :
//! turn bare error variants into caller-actionable diagnostics. Wraps useful
//! corpus context (available commands, aggregate names) into the String
//! payload so the existing variant shape stays untouched but the caller sees
//! what to fix. Pure reads over rt.domain — no dispatch state.
//!
//! Cask extracted VERBATIM from runtime/command_dispatch.rs (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/dispatch_diagnostics.rs — kernel-floor
//!  dispatch error surface, relocated verbatim from command_dispatch.rs
//!  blanket.]

use super::Runtime;

/// List every aggregate visible in the loaded domains, in
/// `Context.Aggregate` form when a context is set so the caller
/// can disambiguate same-name aggregates across bluebooks.
fn available_aggregate_names(rt: &Runtime) -> Vec<String> {
    let mut names: Vec<String> = rt.domain.aggregates.iter()
        .map(|a| match &a.context {
            Some(ctx) => format!("{}.{}", ctx, a.name),
            None => a.name.clone(),
        })
        .collect();
    names.sort();
    names.dedup();
    names
}

/// List every command declared on a specific aggregate (by name),
/// across every bluebook the aggregate appears in. Returned in
/// `Aggregate.Command` form. Includes entity-owned commands as
/// `Aggregate.Entity.Command` so the caller can see the i111-J
/// addressable surface too.
fn commands_on_aggregate(rt: &Runtime, agg_name: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for agg in rt.domain.aggregates.iter().filter(|a| a.name == agg_name) {
        for cmd in &agg.commands {
            out.push(format!("{}.{}", agg.name, cmd.name));
        }
        for ent in &agg.entities {
            for cmd in &ent.commands {
                out.push(format!("{}.{}.{}", agg.name, ent.name, cmd.name));
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// List every command in every aggregate/entity in the corpus, in
/// fully-qualified form. Used as the fallback hint for bare-name
/// dispatch misses where we have no aggregate context to narrow
/// the listing by.
fn all_commands_qualified(rt: &Runtime) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for agg in &rt.domain.aggregates {
        let agg_prefix = match &agg.context {
            Some(ctx) => format!("{}.{}", ctx, agg.name),
            None => agg.name.clone(),
        };
        for cmd in &agg.commands {
            out.push(format!("{}.{}", agg_prefix, cmd.name));
        }
        for ent in &agg.entities {
            for cmd in &ent.commands {
                out.push(format!("{}.{}.{}", agg_prefix, ent.name, cmd.name));
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Truncate a candidates list for the error string. Long
/// listings (1000+ commands across the corpus) would drown the
/// error ; the cap keeps the message useful without hiding
/// information silently — the truncated count is included.
fn join_truncated(items: &[String], cap: usize) -> String {
    if items.len() <= cap {
        items.join(", ")
    } else {
        format!("{}, … ({} more)", items[..cap].join(", "), items.len() - cap)
    }
}

pub(super) fn unknown_aggregate_message(rt: &Runtime, agg_name: &str) -> String {
    let avail = available_aggregate_names(rt);
    if avail.is_empty() {
        format!("{} — no aggregates loaded in this runtime", agg_name)
    } else {
        format!(
            "{} — no aggregate by that name in the loaded domains. Available: {}",
            agg_name,
            join_truncated(&avail, 40),
        )
    }
}

pub(super) fn unknown_command_message_3part(
    rt: &Runtime, full: &str, a: &str, b: &str, c: &str,
) -> String {
    // 3-part form means either Context.Aggregate.Command or
    // Aggregate.Entity.Command. The caller almost certainly
    // intended one of those — list commands on each candidate
    // aggregate so they can self-correct.
    let mut hints: Vec<String> = Vec::new();
    // Context.Aggregate.Command : aggregates named `b` in context `a`.
    for agg in rt.domain.aggregates.iter()
        .filter(|agg| agg.name == b && agg.context.as_deref() == Some(a))
    {
        for cmd in &agg.commands {
            hints.push(format!("{}.{}.{}", a, b, cmd.name));
        }
    }
    // Aggregate.Entity.Command : aggregates named `a` with entity `b`.
    for agg in rt.domain.aggregates.iter().filter(|agg| agg.name == a) {
        for ent in agg.entities.iter().filter(|e| e.name == b) {
            for cmd in &ent.commands {
                hints.push(format!("{}.{}.{}", a, b, cmd.name));
            }
        }
    }
    hints.sort();
    hints.dedup();
    if hints.is_empty() {
        // No aggregate context matched at all — fall back to a
        // corpus-wide listing so they see what IS available.
        let all = all_commands_qualified(rt);
        format!(
            "{} — no command at that address. Looked for Context.Aggregate.Command ('{}.{}') and Aggregate.Entity.Command ('{}.{}'); neither matched. Available commands: {}",
            full, a, b, a, b,
            join_truncated(&all, 30),
        )
    } else {
        format!(
            "{} — '{}' not declared at that address. Did you mean: {}",
            full, c, join_truncated(&hints, 20),
        )
    }
}

pub(super) fn unknown_command_message_2part(
    rt: &Runtime, full: &str, agg_name: &str, cmd_name: &str,
) -> String {
    let on_agg = commands_on_aggregate(rt, agg_name);
    if on_agg.is_empty() {
        // The aggregate part itself doesn't exist.
        let aggs = available_aggregate_names(rt);
        format!(
            "{} — no aggregate '{}' in the loaded domains. Available aggregates: {}",
            full, agg_name, join_truncated(&aggs, 40),
        )
    } else {
        format!(
            "{} — command '{}' not declared on '{}'. Available on this aggregate: {}",
            full, cmd_name, agg_name, join_truncated(&on_agg, 30),
        )
    }
}

pub(super) fn unknown_command_message_bare(rt: &Runtime, cmd_name: &str) -> String {
    let all = all_commands_qualified(rt);
    if all.is_empty() {
        format!("{} — no commands declared in the loaded domains", cmd_name)
    } else {
        format!(
            "{} — no command by that name in the loaded domains. Available: {}",
            cmd_name, join_truncated(&all, 30),
        )
    }
}
