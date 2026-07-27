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

## CLOSED (2026-07-27) — scorecard

The inventory above missed three production sites (deployments/ + cli/
sat outside the parity walk — the "zero real fixture blocks" claim was
wrong). Final ledger, every bite RED→GREEN before its fixture died :

| site | fixture | policies | seam | bite |
|---|---|---|---|---|
| pilot | agent_instrumentation.fixtures | 2 | agent_defs render(corpus_rt) | agent_defs_establishment_test |
| 1 | system_prompt_content.fixtures (miette) | 15 + sections/*.md paths | system_prompt render(corpus_rt), Phase 4 after completion | system_prompt_establishment_test |
| 2 | mindstream.fixtures (miette deploy) | 15 (Define+14 Register w/ :order) | projection::procfile + `project procfile` (Procfile + .overmind.env byte-identical) | mindstream_establishment_test |
| 4 | cloudflare.bluebook inline block | 1 | projection::wrangler + `project wrangler` (wrangler.toml byte-identical — Chris: "we need a seam") | worker_config_establishment_test |
| 5 | daily_musing.fixtures (worker seeds) | 2 | worker lib.rs dispatches CompleteBoot per boot | daily_musing_establishment_test |
| 6 | subcommand.fixtures (30 rows) | 30 | seed-subcommand-registry → `storehouse establish cli/subcommand/bluebook` | subcommand_registry_establishment_test |

Orphans deleted : 24 (prior sweep) + 15 (repo-level fixtures/ dirs the
runner never probes) + transparency.fixtures (miette). Language surface
gone : grammar line, BlockParser::Fixture + routing, parser arm +
consume_do_block, Domain.fixtures IR, dump/canonical_ir keys, server
views (html_fixtures/html_kpi deleted), conceiver dim (9→8), Ruby
builder stub + aggregate-scope no-op ; `fixture` is asserted unlinkable
in block_grammar_test. Zero `fixture` blocks parse anywhere ; remaining
.fixtures are behaviors-runner siblings or retained test vocabulary
(parity/fixtures/* — the fixtures-parser harness corpus ;
rust/tests/fixtures/vindiction ; rem_dream canned :fixture-llm responses).

Findings filed while here — ALL CLOSED (2026-07-27, same session) :
- miette's 2 pre-existing behaviors failures (proprioception SenseLimb,
  being GraftDomain) → kernel apply_defaults branch-order bug : a
  list_of(X) whose element VO carries member defaults initialised as the
  VO's Map. Fixed list-first (7e5dac48f) ; pinned by list_vo_default_test.
- unknown TOP-LEVEL block keywords → recorded on Domain.unknown_keywords,
  whole block consumed (the inner-line leak was a latent parity hole —
  Ruby never evaluates an unknown method's block), validator reports
  INVALID with `did you mean` on near-misses (773b0eef2). Accepted-but-
  uncaptured keywords (saga/glossary/…) consume silently ; paragraph
  stays transparent.
- corrupted all-empty Subcommand row (id 1) → retired through the door
  (SubcommandRegistry::Subcommand.Retire), 47→46 records.
