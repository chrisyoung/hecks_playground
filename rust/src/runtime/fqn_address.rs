//! fqn_address — the FQN address GRAMMAR, pure string functions with no
//! Runtime : parse_fqn (end-reading of variable-depth realm-qualified
//! addresses), realm_over_qualified_tail (the over-qualified reduction
//! fallback), and ambiguity_candidates (the realm-strict guard of the FQN
//! flip). Corpus-backed resolution stays in command_dispatch.rs ; the
//! tests live in fqn_tests.rs.
//!
//! Cask extracted VERBATIM from runtime/command_dispatch.rs (cask-runtime).
//!
//! [antibody-exempt: rust/src/runtime/fqn_address.rs — kernel-floor dispatch
//!  address grammar, relocated verbatim from command_dispatch.rs blanket.]

/// Parse a realm-qualified, VARIABLE-DEPTH address into component parts.
///
/// The folder tree IS the namespace : `Realm[::Subrealm…]::Domain::Aggregate.command`
/// (or `.query_name`). Parsed by ENDS, not count — the last `::` segment is the
/// Aggregate, the second-to-last is the Domain, and everything before them
/// (Realm + 0+ subrealms) is the namespace path. Minimum 2 segments.
///
/// Returned tuple: (domain, aggregate, command_or_query) — the two address ENDS
/// that resolution keys on. The Realm/subrealm prefix is ACCEPTED here ; during
/// the migration bridge it is not yet enforced in resolution (that arrives with
/// the realm-path stamped on each Aggregate). So both the new
/// `Realm::Domain::Aggregate` form and the legacy prefix-free `Domain::Aggregate`
/// (i560 v2) resolve identically — the latter is just the zero-prefix case.
pub fn parse_fqn(command_name: &str) -> Result<(String, String, String), String> {
    let err = || format!(
        "calling format is Realm::Domain::Aggregate.command (queries: .query_name lowercase) — got '{}' (need at least Domain::Aggregate before the '.')",
        command_name
    );
    let (head, tail) = match command_name.rsplit_once('.') {
        Some(pair) => pair,
        None => return Err(err()),
    };
    if tail.is_empty() || head.is_empty() {
        return Err(err());
    }
    let segments: Vec<&str> = head.split("::").collect();
    if segments.len() < 2 || segments.iter().any(|s| s.is_empty()) {
        return Err(err());
    }
    let n = segments.len();
    Ok((
        segments[n - 2].to_string(), // Domain  — second-to-last segment
        segments[n - 1].to_string(), // Aggregate — the leaf (last segment)
        tail.to_string(),
    ))
}

/// Decide whether a multi-hit resolution is AMBIGUOUS — the pure core of the
/// FQN flip, extracted so it is unit-testable without a Runtime. A dispatch
/// that OMITTED its realm (`realm_omitted`) and resolves to >1 DISTINCT
/// non-empty realm_path is ambiguous : returns the sorted candidate FQNs the
/// caller must disambiguate between. Returns None when the dispatch carried a
/// realm, when fewer than two distinct realms remain, or when the matching
/// aggregates carry no stamped realm_path (legacy / string-parsed) — those
/// keep first-match-wins, exactly as before the flip.
pub(super) fn ambiguity_candidates(hit_realms: &[String], realm_omitted: bool, command_name: &str) -> Option<Vec<String>> {
    if !realm_omitted {
        return None;
    }
    let mut candidates: Vec<String> = hit_realms.iter()
        .filter(|rp| !rp.is_empty())
        .map(|rp| format!("{}::{}", rp.replace('/', "::"), command_name))
        .collect();
    candidates.sort();
    candidates.dedup();
    if candidates.len() > 1 {
        Some(candidates)
    } else {
        None
    }
}

/// Resolve a command address to a Resolution (aggregate or entity-owned).
///
/// ## Canonical form — i560 v2 FQN migration (2026-05-12)
///
///   - `Domain::Aggregate.Command` (commands, PascalCase)
///   - `Domain::Aggregate.query_name` (queries, snake_case)
///
///     Two `::`-separated segments followed by `.<Command-or-query>`.
///
///     - `Domain` matches against an aggregate's `context` (bluebook
///       namespace, set by `Hecks.bluebook "X"`) OR against the
///       bluebook's `category` (the directory under `aggregates/`,
///       e.g. `discipline`, `framework`, `world`). Case-insensitive.
///     - `Aggregate` matches the aggregate's `name`.
///     - The post-`.` token is PascalCase for commands and snake_case
///       for queries.
///
///   This is the form the CLI accepts (`storehouse <root>
///   Domain::Aggregate.Command`). The CLI gate in `main.rs` rejects
///   the legacy short forms with a helpful error naming the canonical
///   shape. The resolver below still accepts the legacy dotted forms
///   so internal cascade machinery (`drain_policies`,
///   `dispatch_cascade`) keeps working — bluebook `trigger_command`
///   strings are typically declared as `Aggregate.Command` in source
///   today, and rewriting every bluebook is out of scope for this
///   sidequest.
///
/// ## Legacy forms (accepted for internal cascade use)
///
///   - `Context.Aggregate.Command` — three dotted parts (i142 form).
///   - `Aggregate.Entity.Command` — three dotted parts that don't
///     match a known context (i111-J).
///   - `Aggregate.Command` — two dotted parts.
///   - `Command` — bare command name.
/// Reduce a realm-OVER-qualified FQN to its canonical `Domain::Aggregate.Command`
/// tail — the last TWO `::`-delimited namespace segments before the `.command`.
/// `Hecks::Framework::Cascade::Cascade.RecordResult` -> `Cascade::Cascade.RecordResult`.
/// Returns None when the address is already two segments or fewer (nothing to
/// strip) or carries no `.command`, so the caller only retries a genuinely
/// over-qualified target. The realm-tolerant fallback for hecksagon `result_into`
/// dispatch targets (2026-06-30).
pub(super) fn realm_over_qualified_tail(command_name: &str) -> Option<String> {
    let dot = command_name.rfind('.')?;
    let addr = &command_name[..dot];
    let cmd = &command_name[dot..]; // includes the leading '.'
    let segs: Vec<&str> = addr.split("::").collect();
    if segs.len() <= 2 {
        return None;
    }
    Some(format!("{}::{}{}", segs[segs.len() - 2], segs[segs.len() - 1], cmd))
}
