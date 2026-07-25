//! fqn_tests — the FQN address grammar under test : parse_fqn end-reading
//! (variable-depth realm prefixes), ambiguity_candidates (the realm-strict
//! guard), caller-scoped local-first resolution, and the over-qualified
//! tail reduction. Exercises command_dispatch's resolution surface only —
//! no dispatch side effects.
//!
//! Cask extracted VERBATIM from runtime/command_dispatch.rs (cask-runtime) ;
//! `super::X` became `command_dispatch::X`, nothing else changed.
//!
//! [antibody-exempt: rust/src/runtime/fqn_tests.rs — kernel-floor dispatch
//!  resolution tests, relocated verbatim from command_dispatch.rs blanket.]

use super::command_dispatch;
use super::command_dispatch::parse_fqn;


// parse_fqn reads a realm-qualified, VARIABLE-DEPTH address by its ENDS :
// last :: segment = Aggregate, second-to-last = Domain, everything before
// (Realm + 0+ subrealms) is the accepted-but-unenforced namespace prefix.
// Returns (domain, aggregate, verb).

#[test]
fn legacy_two_segment_is_the_zero_prefix_case() {
assert_eq!(
    parse_fqn("AgentInbox::AgentMessage.AllUnread").unwrap(),
    ("AgentInbox".to_string(), "AgentMessage".to_string(), "AllUnread".to_string())
);
}

#[test]
fn realm_qualified_three_segment_resolves_by_ends() {
assert_eq!(
    parse_fqn("Hecks::AgentInbox::AgentMessage.all_unread").unwrap(),
    ("AgentInbox".to_string(), "AgentMessage".to_string(), "all_unread".to_string())
);
}

#[test]
fn subrealm_four_segment_still_takes_the_two_ends() {
assert_eq!(
    parse_fqn("Hecks::Framework::AgentInbox::AgentMessage.all_unread").unwrap(),
    ("AgentInbox".to_string(), "AgentMessage".to_string(), "all_unread".to_string())
);
}

#[test]
fn deep_n_segment_uncapped() {
assert_eq!(
    parse_fqn("A::B::C::D::E::Agg.Cmd").unwrap(),
    ("E".to_string(), "Agg".to_string(), "Cmd".to_string())
);
}

#[test]
fn malformed_addresses_are_rejected() {
assert!(parse_fqn("NoColons.Cmd").is_err());   // < 2 :: segments
assert!(parse_fqn("Just::Two").is_err());       // no '.' verb
assert!(parse_fqn("Dangling::.Cmd").is_err());  // empty trailing segment
assert!(parse_fqn("::Agg.Cmd").is_err());       // empty leading segment
}

// ---- the FLIP : ambiguity_candidates (the realm-strict guard) ----

#[test]
fn ambiguity_two_distinct_realms_realm_omitted_is_ambiguous() {
// The flip's core : a realm-OMITTED 2-seg dispatch hitting the same
// Domain::Aggregate.command across two realms reports BOTH candidates
// instead of silently first-match-winning.
let realms = vec!["hecks/framework".to_string(), "miette/body".to_string()];
let got = command_dispatch::ambiguity_candidates(&realms, true, "Tools::Widget.Make");
assert_eq!(got, Some(vec![
    "hecks::framework::Tools::Widget.Make".to_string(),
    "miette::body::Tools::Widget.Make".to_string(),
]));
}

#[test]
fn ambiguity_realm_qualified_dispatch_is_never_ambiguous() {
// When the caller named the realm, realm_context_matches already
// narrowed — keep first-match-wins.
let realms = vec!["hecks/framework".to_string(), "miette/body".to_string()];
assert_eq!(command_dispatch::ambiguity_candidates(&realms, false, "X::Y.Z"), None);
}

#[test]
fn ambiguity_same_realm_twice_is_not_ambiguous() {
// Two hits in the SAME realm dedup to one candidate — not ambiguous.
let realms = vec!["hecks/framework".to_string(), "hecks/framework".to_string()];
assert_eq!(command_dispatch::ambiguity_candidates(&realms, true, "X::Y.Z"), None);
}

#[test]
fn ambiguity_unstamped_realms_keep_first_match() {
// Legacy / string-parsed aggregates carry an empty realm_path —
// filtered out, so they fall through to first-match-wins.
let realms = vec![String::new(), String::new()];
assert_eq!(command_dispatch::ambiguity_candidates(&realms, true, "X::Y.Z"), None);
}

// ---- caller-scoped local-first (i-fqn local-short) ----

// A "Widget.Make" homonym across two bluebooks (hecks + miette), both with
// context "Framework" so the 2-seg `Framework::Widget.Make` form hits BOTH.
fn two_widget_domain() -> crate::ir::Domain {
let src = r#"
Hecks.bluebook "Framework" do
aggregate "Widget" do
attribute :id, Id
command "Make" do
  attribute :id, Id
end
end
end
"#;
let mut d = crate::parser::parse(src);
for a in d.aggregates.iter_mut() { a.realm_path = Some("hecks/framework".to_string()); }
let mut other = d.aggregates.iter().find(|a| a.name == "Widget").unwrap().clone();
other.realm_path = Some("miette/framework".to_string());
d.aggregates.push(other);
d
}

#[test]
fn caller_scope_picks_local_dotted_form() {
let rt = crate::runtime::Runtime::boot(two_widget_domain());
// Dotted Aggregate.Command : caller in hecks resolves the hecks Widget …
let r = command_dispatch::resolve(&rt, "Widget.Make", Some("hecks/framework")).unwrap();
assert_eq!(rt.domain.aggregates[r.agg_idx()].realm_path.as_deref(), Some("hecks/framework"));
// … the SAME short ref from miette resolves the miette Widget.
let r = command_dispatch::resolve(&rt, "Widget.Make", Some("miette/framework")).unwrap();
assert_eq!(rt.domain.aggregates[r.agg_idx()].realm_path.as_deref(), Some("miette/framework"));
}

#[test]
fn caller_scope_picks_local_two_seg_form() {
let rt = crate::runtime::Runtime::boot(two_widget_domain());
// 2-seg Bluebook::Aggregate.Command hits both ; caller scope disambiguates
// instead of erroring AmbiguousCommand or first-match-guessing.
let r = command_dispatch::resolve(&rt, "Framework::Widget.Make", Some("miette/framework")).unwrap();
assert_eq!(rt.domain.aggregates[r.agg_idx()].realm_path.as_deref(), Some("miette/framework"));
}

#[test]
fn no_caller_scope_falls_back_to_first_match() {
let rt = crate::runtime::Runtime::boot(two_widget_domain());
// ADDITIVE : no caller scope → today's global first-match (hecks declared first).
let r = command_dispatch::resolve(&rt, "Widget.Make", None).unwrap();
assert_eq!(rt.domain.aggregates[r.agg_idx()].realm_path.as_deref(), Some("hecks/framework"));
}

#[test]
fn cascade_short_ref_matching_only_foreign_errors() {
let rt = crate::runtime::Runtime::boot(two_widget_domain());
// TIGHTEN : a CASCADE (scoped) short ref whose only matches live in OTHER
// bluebooks (both Widgets are stamped, neither in this scope) is an error —
// a cross-bluebook cascade must use the full FQN, not a first-matched short.
let r = command_dispatch::resolve(&rt, "Widget.Make", Some("hecks/elsewhere"));
assert!(r.is_err(), "expected foreign-only cascade short ref to error, got {:?}", r);
}

#[test]
fn cascade_two_seg_foreign_only_errors_but_local_resolves() {
let rt = crate::runtime::Runtime::boot(two_widget_domain());
// 2-seg short ref from a scope where NO Widget lives → spans only foreign
// bluebooks → tighten errors.
let r = command_dispatch::resolve(&rt, "Framework::Widget.Make", Some("hecks/elsewhere"));
assert!(r.is_err(), "expected foreign-only 2-seg cascade ref to error, got {:?}", r);
// … but the SAME ref from a local scope still resolves (local-first intact).
let r2 = command_dispatch::resolve(&rt, "Framework::Widget.Make", Some("miette/framework")).unwrap();
assert_eq!(rt.domain.aggregates[r2.agg_idx()].realm_path.as_deref(), Some("miette/framework"));
}

// A single Widget stamped in ONE bluebook (no homonym).
fn one_widget_domain() -> crate::ir::Domain {
let src = r#"
Hecks.bluebook "Framework" do
aggregate "Widget" do
attribute :id, Id
command "Make" do
  attribute :id, Id
end
end
end
"#;
let mut d = crate::parser::parse(src);
for a in d.aggregates.iter_mut() { a.realm_path = Some("miette/body/organs".to_string()); }
d
}

#[test]
fn cascade_unique_foreign_short_ref_resolves() {
// REGRESSION (pulse-pm cross-bluebook fix) : a CASCADE (scoped) short ref
// whose only match is a UNIQUE foreign stamped aggregate must RESOLVE, not
// error — the cross-bluebook PM `dispatch` path (Synapse.CreateSynapse from
// the Pulse PM, event source in body/cycles, target in body/organs). Only a
// genuine homonym across 2+ bluebooks errors.
let rt = crate::runtime::Runtime::boot(one_widget_domain());
let r = command_dispatch::resolve(&rt, "Widget.Make", Some("miette/body/cycles")).unwrap();
assert_eq!(rt.domain.aggregates[r.agg_idx()].realm_path.as_deref(), Some("miette/body/organs"));
}

// ── realm_over_qualified_tail : the realm-tolerant fallback's core (2026-06-30) ──
// Reduces a realm-OVER-qualified dispatch target to its canonical
// Domain::Aggregate.Command tail. The live regression : a hecksagon
// `result_into` of `Hecks::Framework::Cascade::Cascade.RecordResult` derived a
// realm/context no aggregate's realm_path matched, so the strict FQN resolve
// rejected it and EVERY tool's outcome-chain cascade silently failed. The
// fallback retries the tail, which resolves.

#[test]
fn over_qualified_fqn_reduces_to_domain_aggregate_command_tail() {
assert_eq!(
    command_dispatch::realm_over_qualified_tail("Hecks::Framework::Cascade::Cascade.RecordResult"),
    Some("Cascade::Cascade.RecordResult".to_string())
);
// A deeper realm still reduces to the LAST two namespace segments.
assert_eq!(
    command_dispatch::realm_over_qualified_tail("A::B::C::D::Widget.Make"),
    Some("D::Widget.Make".to_string())
);
}

#[test]
fn already_minimal_fqn_is_not_reduced() {
// Exactly two segments (Domain::Aggregate) — nothing to strip.
assert_eq!(command_dispatch::realm_over_qualified_tail("Cascade::Cascade.RecordResult"), None);
// One segment before the command — also minimal.
assert_eq!(command_dispatch::realm_over_qualified_tail("Widget.Make"), None);
// No command at all — no tail to build.
assert_eq!(command_dispatch::realm_over_qualified_tail("A::B::C::Widget"), None);
}

