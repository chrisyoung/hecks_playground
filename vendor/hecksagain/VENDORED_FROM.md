# Vendored from

This directory's `lib/` and `hecksagain.gemspec` are vendored (plain copy,
no submodule/subtree) from:

- **Fork**: https://github.com/chrisyoung/hecks-hecksagain
- **Branch**: `main`
- **Commit**: `1cce2d10cea576efd75ff92be81f5b020878d486`
- **Vendored**: 2026-08-18 (re-vendor #3)

`rust/` and `storehouse/` are explicitly NOT vendored here and never have
been — only `lib/` + `hecksagain.gemspec`. The `bin/` and `rust/`
directories that sit beside this file are older, unrelated content, not
part of any vendoring pass.

## THIS DIRECTORY IS NOW THE REAL SOURCE, not a fallback

`hecks`'s `Gemfile` sources hecksagain as `gem "hecksagain", path:
"vendor/hecksagain"`. It used to be a `git:`/`ref:` pin straight at the
fork — but the fork is PRIVATE, and `ci.yml` (pure Ruby, no Rust) has no
credentials to clone it: every CI run died in `bundle install` before a
single spec ran. A `path:` source needs no token, no network, and no
private-repo access at all.

So: **bump by re-vendoring, never by hand-editing files in here.** Copy
the fork's `lib/` + `hecksagain.gemspec` again, 3-way merge as described
below, update this note, `bundle install`, re-run the verification.

## What changed since the last vendor (`835435154b` -> `1cce2d1`)

`835435154b` **is not an ancestor of `1cce2d1`.** The fork's `main` was
squash-and-reapplied onto real upstream hecksagain's `main` between the
two, so this is not a fast-forward diff — it is a re-baselining. Against
the old tree: **279 files changed, +18,740 / -6,323** across `lib/` +
the gemspec.

**Structural, from the rebase onto upstream:**

- `bluebook/ir/*` (19 files) collapsed into a single `ir.rb`; the
  behaviors IR moved from `bluebook/ir/behaviors.rb` to
  `bluebook/behaviors.rb` (namespace `Bluebook::TestCase` etc., no longer
  under `Bluebook::IR`).
- `presentation/` (14 files) retired, replaced by `forms/` + `forms.rb`
  and `projections/` + `projections.rb`.
- New top-level modules: `vocabulary.rb`, `freezer.rb`, `literal.rb`,
  `embryonaut_bluebook.rb`, `ir.rb`.
- `adapters/driven/postgres/lineage*` reshaped; `query_specification/
  common/*` dropped; `adapters/driven/claude_code.{rb,adapter}` and
  `adapters/driven/heki/saga_store.rb` added.

**Behaviour landed since (the reason for this bump):**

- `1cce2d1` — **`for_each:` fan-out on a hecksagon's `driven` dispatch**
  (fork PR #8). One event fans a command out over every record a query
  matches. This is the whole point of the re-vendor: it is what i787's
  corpus-wide `with: {}` migration exists to use.
- `74ea7da` — `record_effect_outbound`, the runtime producer for the
  spawn/`charged_by` effect port's `Bind#on/success/failure` DSL capture
  (i780/i781's `:exec` -> `spawned_by` re-expression consumes it).
- `fe39d44` — `AdapterBuilder#handler` was never wired; every `.adapter`
  file using the `handler "..."` DSL word raised NoMethodError at parse.
- `933d1dd` / `73f1792` — `Hecks.boot(path, environment:)`: swappable
  per-environment wiring as a real file.
- `4cec55d` / `4ba551c` — payload gate: unwrap a VO discriminant before
  the `one_of` vocabulary check; judge `list_of(VO)` `one_of` attributes
  per element.
- `e93841e` — parser recognises single-line curly `identified_by { X.value }`.
- `a0352ba` — single-field VO defaults unwrapped through lifecycle +
  equality.
- `a195dbe` / `31e085e` — behaviors-root discovery: requires a co-located
  `.git`, and recognises decoupled being repos.
- `27be5c7` / `8565f08` — arithmetic: `increment`/`decrement`/`multiply`
  on an absent VO-typed attribute no longer blame the amount; refuse when
  a compound value's own field disagrees with its source.
- `b7bac14`..`493ad3d` — `EmbryonautBluebook` (external bluebook loader)
  plus the `uses_embryonaut_bluebook` DSL word, declared in the
  self-hosted grammar and documented.
- Re-applied fork-only work now upstream in the fork: the execution port
  + Shell/Filesystem/Search/Email driven adapters, the `.behaviors`
  authoring DSL + IR, driven-handler dispatch, R2 persistence, the
  `AppendLog`/`DiskBuffer`/`DreamImage`/`Stripe` adapter registrations,
  registry hecksagon-merge for a flattened multi-file corpus.

## Local-only additions: NONE remain (checked, not assumed)

Re-vendor #2 carried twelve files that existed only in this directory and
not on the fork (the execution port, the `.behaviors` DSL + IR, and the
Shell/Filesystem/Search/Email driven adapters). **All twelve have since
been pushed to the fork and are present in `1cce2d1`** — verified file by
file:

- The 8 driven-adapter files (`{email,filesystem,search,shell}.{rb,adapter}`)
  and `ports/execution.port` are byte-identical to what was vendored here.
- `bluebook/dsl/behaviors_builder.rb`, `ports/execution.rb`, and the
  behaviors IR are supersets — same code plus upstream's own doc headers,
  the `Bluebook::IR::` -> `Bluebook::` namespace move, and a real fix in
  `Execution#unwrap` for a raw `{ value: ... }` hash arriving from a
  dispatch's uncoerced kwargs.

Set-difference check: of the 46 files present here and absent from
`1cce2d1`, 45 came from the OLD fork commit and were deliberately removed
by the restructure above. Exactly one — `bluebook/ir/behaviors.rb` — was
genuinely local, and it is the file relocated to `bluebook/behaviors.rb`.

So the 3-way merge (base = `835435154b`'s `lib/`+gemspec, ours = this
directory's prior content, theirs = `1cce2d1`) collapses to a straight
copy of `1cce2d1`. That was the conclusion of the check, not the
assumption going in. `hecksagain.gemspec` was already byte-identical.

## One deliberate divergence from the fork's `lib/`

`lib/hecksagain/framework/bluebook/compliance.bluebook` is a **symlink**
in the fork, pointing at `../../../../examples/compliance/bluebook/
compliance.bluebook` — a path that escapes `lib/`. In a full checkout of
the fork (which is what a `git:` Bundler source gets) it resolves fine.
In a `lib/`-only vendoring it dangles, and `Framework.members` globs that
directory, so an `uses_framework "Compliance"` would find an entry it
could never `Kernel.load`.

It is **materialised here as a regular file** with the symlink's exact
target content from `1cce2d1`, so `Framework.members` behaves identically
to the git source. Nothing in `hecks_conception` attaches Compliance
today; this is closing the hole before something does. Re-materialise it
the same way on the next re-vendor. No other symlinks exist under `lib/`.

## Verification (2026-08-18)

- `bundle show hecksagain` -> this directory; `Gemfile.lock` has no `GIT`
  section at all, so `bundle install` needs no private-repo credentials.
- **Validate sweep unchanged**: `HecksagainRuntime.validate("hecks_conception")`
  reports **63 roots swept, 59 valid, 4 invalid** — byte-for-byte the same
  baseline the `1cce2d1` git pin produced, down to the same four
  pre-existing failures (`agent_inbox` + `event_sourcing` on
  `driving on interval`, `tools` + `storehouse` on adapter arity).
- **Live fan-out re-proved** against a disposable staged copy of the real
  Conductor domain (`bluebook/` + `hecksagons/` flattened into one dir —
  a passthrough boot of the domain root does NOT load `hecksagons/`):
  `Worker.MarkDead` -> `WorkerDied` fans `Claim.Expire` over 3 held claims,
  all 3 land `expired`. With the block's second dispatch removed so
  `Lease.Reclaim` is the only one, it fans over 3 active leases and all 3
  land `reclaimed`. That split is the KNOWN, already-documented gem gap
  (a `driven` block holds a single `@dispatch_command`, so only the LAST
  of two registers — see `f5557d80b`), not a vendoring regression.
- The de-over-qualified verb strings (i787, `f5557d80b`) are proved by the
  same run: the driven handler only matches because the hecksagon now says
  bare `Conductor::Worker.WorkerDied`.
- `bundle exec rspec` (what `ci.yml` runs) finds 0 examples — CI's only
  real gate here was `bundle install`, which is exactly what this change
  fixes.

## Known gap, inherited and NOT re-checked this round

Re-vendor #2 recorded 39 failing examples in the FORK's own
`bundle exec rspec`, all in hecksagain's self-hosting/golden/coverage-parity
meta-tests (`ir_golden_spec`, `syntax_conformance_spec`, `dsl_coverage_spec`,
`judge_coverage_spec`, `vocabulary_conformance_spec`, `plurality_coverage_spec`,
`projector_spec`, `reference_golden_spec`, `round_trip_spec`, `guides_spec`).
Those assert every DSL construct is documented, unit-tested, and reflected
in frozen golden IR snapshots. The rebase onto upstream `main` may well
have changed that count in either direction; this pass did not re-run the
fork's suite, so treat the number as stale. None of it indicates broken
runtime behaviour — the sweep and live-fire above are the real gate.
Closing it needs a `bin/evolve`-style pass on the fork itself.
