# fixtures → policies — inventory + conversion design (2026-07-26)

Goal (Chris): production fixtures become policies ; fixtures remain only
in tests ; `fixtures` leaves the bluebook language ; behavior identical.
Discipline (Chris): each policy must BITE before its fixture dies — a
test red without the policy, green with it, then the fixture is removed.

## The blessed pattern (already in the codebase)

Establishment policies `on "BootCompleted"` (run_boot/complete.rs, the
boot-establishment keystone) : idempotent upserts on natural keys via
EXISTING commands + `with` literals ; `storehouse establish <root>`
re-asserts on demand. Precedent : the authz roster
(authorization.bluebook RosterAssign* policies).

## Inventory

### Production fixtures (CONVERT, one biting test each)
1. **agent_instrumentation.fixtures** (3 AgentDefinition rows) — consumed
   by run_boot/agent_defs.rs (phase 4b writes .claude/agents defs).
   AgentDefinition has identified_by :name + Register — policies drop in.
   ORDERING: phase 4b runs BEFORE establishment today ; the projection
   must read established records, so it moves after complete_boot (or
   re-runs there).
2. **system_prompt_content.fixtures** (miette repo) — consumed by
   run_boot/system_prompt.rs (SystemPromptSection rows → the prompt).
   Same shape : Establish policies in the being's system_prompt bluebook,
   runner reads established records. Cross-repo (miette) — needs the
   bluebook there + hecks runner rewire.
3. **mindstream.fixtures** (miette deploy) — MindstreamMember rows ;
   consumers: cold-setup.sh / Procfile derivation (i276 gap), statusline?
   Convert to establishment; derive Procfile from established records.

### Test fixtures (STAY — the behaviors runner's vocabulary)
- plan.fixtures, loc_ratchet.fixtures, agent_discipline.fixtures (flat
  siblings of .behaviors, auto-loaded by behaviors_fixtures.rs).
- fixtures_parser.rs + behaviors_fixtures.rs + fixtures_ir.rs stay.
- llm :fixture backend (canned responses) — test vocabulary, stays.

### Orphans (NO consumer found — verify dead, then delete)
- aggregates/fixtures/*.fixtures ×18 (awareness, being, bluebook, boot,
  circuit_breaker, first_breath, inference, interpretation, memory,
  organs, scale, sleep, spend, system_prompt, tongue, training_corpus,
  training_pipeline, vocabulary, vows) — legacy seeds, no loader
  anywhere in rust/ruby/bin/miette.
- catalog/fixtures/{boot,law,court}.fixtures, catalog/applications/rvdc,
  adapters/fixtures/ollama.fixtures — same: no consumer matched.
- Proof-of-death = full test suite + boot + behaviors green after removal.

### Language removal (vestigial — NO bluebook uses fixture syntax today)
All 16 grep hits in .bluebook files are prose/attribute names, zero real
blocks. Remove: BlockParser::Fixture + consume_do_block routing
(parser.rs), parse_fixture + parse_fixture_block (parse_blocks.rs),
Domain.fixtures IR field, server/html_fixtures + html_kpi/usage/domain
readers, conceiver/vector fixture dimension, run_restructure check,
Ruby parser parity twin, canonical_ir/known_drift entries, grammar
bluebook prose. Gate: parity suite + full tests green.

## Order of work
1. Pilot: site 1 (agent defs) — policy + biting behaviors test + runner
   rewire + fixture deletion, committed green.
2. Sites 2-3 (miette repo bluebooks + runner rewires).
3. Orphan deletions (prove nothing bites: suite green before/after).
4. Language removal sweep + parity.
