# Vendored from

This directory's `lib/` and `hecksagain.gemspec` are vendored (plain copy,
no submodule/subtree) from:

- **Fork**: https://github.com/chrisyoung/hecks-hecksagain
- **Branch**: `main`
- **Commit**: `db253cd2b33369bb23e807b33832984594b5c7fe`
- **Vendored**: 2026-08-11

The fork is based on the real upstream `main` branch of
https://github.com/chrisyoung/hecksagain (not the `feat/interview-bluebook`
branch the original, un-forked vendoring pass mistakenly copied from —
see the fork's own git history for the correction), plus ten commits
carrying every fix this migration needed on top: expression-resolver
Ruby-idiom methods (`.match?`/`.present?`/`.blank?`/etc), the
self-hosted `redirects_native` governed-door fix, query DSL growth
(`count`/`median`/`group_by`/`none_in_state`), policy `where`/`for_each`,
saga memory/`given`/`template`, new mutation ops (`remove`/`multiply`/
`clamp`) with Float arithmetic, the driving-adapter grammar plus a long
tail of DSL builder catch-all stubs, staging/registry fixes for a
flattened multi-file corpus, and several adapter/IR additions
(`AppendLog`, `DiskBuffer`, `DreamImage`, `Stripe`, `category`, driving
handler wire shape).

**To re-vendor after the fork moves forward**: diff the fork's `lib/` +
`hecksagain.gemspec` against this directory, review, copy, update the
commit hash above.

**Known gap, not yet closed**: the fork's own `bundle exec rspec` has 39
failing examples, all in hecksagain's self-hosting/golden/coverage-parity
meta-tests (`ir_golden_spec`, `syntax_conformance_spec`,
`dsl_coverage_spec`, `judge_coverage_spec`, `vocabulary_conformance_spec`,
`plurality_coverage_spec`, `projector_spec`, `reference_golden_spec`,
`round_trip_spec`, `guides_spec`, plus one runtime spec asserting the
now-deliberately-widened Integer-only arithmetic refusal) — these assert
every DSL construct is documented, unit-tested, and reflected in frozen
golden IR snapshots, and haven't been regenerated for the new surface
added on top of upstream `main`. None of them indicate broken runtime
behavior — `hecksagain-cli validate`/`dispatch`/`query`/`governed-door`
against real corpora (`hecks_conception`, `bin-buddy`, `miette`) all
match expected, previously-verified results. Closing this gap needs a
real `bin/evolve`-style pass (golden fixture regeneration, docs,
coverage specs) on the fork — out of scope for the migration that
produced this vendoring pass.
