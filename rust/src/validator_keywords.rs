//! validator_keywords.rs — the unknown-block-keyword check.
//!
//! A `<word> ".." do` line inside an aggregate body whose word is a near-miss
//! of a real block keyword (`commnd` → `command`) is a typo : the parser used
//! to silently walk past the whole block, dropping the command/query/etc. from
//! the IR and leaving validate to complain about an UNRELATED symptom ("A has
//! no commands") — the scout's #1 silent-failure gripe. The parser now records
//! those near-misses on `Aggregate::unknown_keywords` (edit-distance ≤ 2 to a
//! known opener) ; this rule surfaces each as a graceful INVALID with the
//! `did you mean` suggestion, so the newcomer sees the ACTUAL problem.
//!
//! Per-aggregate + per-file : a typo is a typo regardless of corpus, so this
//! runs UNCONDITIONALLY in every validate path (bare, batch, corpus).
//!
//! Hand-written framework source — outside the specializer targets
//! (validator.rs / validator_warnings.rs), the same home as
//! validator_corpus.rs / validator_inside_refs.rs / validator_mutations.rs.
//!
//! Usage :
//!   let errors = unknown_keyword_errors(&domain);
//!   // empty for a bluebook whose every block opener is a real keyword.

use crate::ir::Domain;

/// Every typo'd block-opener keyword the parser recorded on an aggregate.
/// Reported INVALID-grade with a `did you mean` hint pointing at the nearest
/// real keyword.
pub fn unknown_keyword_errors(domain: &Domain) -> Vec<String> {
    let mut errors = vec![];
    // Domain-level : an unknown TOP-LEVEL block opener (fixtures→policies,
    // 2026-07-27). Unlike the aggregate near-miss rule below, this reports
    // FAR words too — Ruby raises NoMethodError on any of them, and before
    // this rule the Rust parser silently swallowed the whole block (a
    // leftover `fixture` block simply vanished). With a near-miss the
    // message carries the `did you mean` hint ; without one it names the
    // construct as outside the language.
    for uk in &domain.unknown_keywords {
        if uk.suggestion.is_empty() {
            errors.push(format!(
                "unknown top-level keyword `{}` opens a block — not part of the bluebook language ; the whole block is ignored",
                uk.keyword
            ));
        } else {
            errors.push(format!(
                "unknown top-level keyword `{}` opens a block — did you mean `{}` ? the whole block is ignored",
                uk.keyword, uk.suggestion
            ));
        }
    }
    for agg in &domain.aggregates {
        for uk in &agg.unknown_keywords {
            errors.push(format!(
                "aggregate {} : unknown keyword `{}` opens a block but is not a recognised keyword — did you mean `{}` ?",
                agg.name, uk.keyword, uk.suggestion
            ));
        }
    }
    errors
}
