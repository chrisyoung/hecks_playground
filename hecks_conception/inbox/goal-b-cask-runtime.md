# Goal B: Cask-ify the runtime — no file over 200 lines, one intent each

> Axis: GRANULARITY (one god-file -> many concern-casks). Sibling goal:
> goal-a-adapter-resolvers-to-bluebook.md (domain). Run AFTER goal A.

## Objective
Break `rust/src/runtime/mod.rs` (~3,625 after goal A) into concern-casks, each
<=200 code lines with one clear intent, until `mod.rs` is a ~190-line module root
(index + `Runtime` struct + thin front doors) and NO file under `src/runtime/`
exceeds 200 code lines. **Pure relocation — zero behavior change.**

## What this is NOT (read before measuring success)
Not dissolution. `core_runtime`'s total LoC barely moves; code relocates between
files, it does not become bluebook (that is goal A). **loc-ratchet delta ~= 0, no
override needed.** The win is INTENT-PER-FILE, not line reduction. A reviewer
expecting the ratchet to drop is watching the wrong axis.

## Mechanism (proven, not new)
`impl Runtime` blocks in sibling child modules reaching private fields via
`use super::*` — exactly what `query.rs` / `event_sourcing.rs` /
`framework_substrate.rs` already do.

## The seam discipline (makes small files BETTER, not spaghetti)
- Split by CONCERN, never by line count. A cask owns a nameable responsibility,
  not "the next 180 lines".
- The interpreter methods share private state (`repositories`, `event_bus`,
  `outbox`, `pm_engine`, `policy_engine`). A cut across tight coupling produces
  `super::` reach-arounds worse than the monolith. If a clean seam is not there,
  DO NOT cut — leave those methods together and note why.
- One-concern helpers move WITH their concern (`pub(super)`); many-concern helpers
  go to a shared `runtime_util` cask, never duplicated.

## The exemption becomes precise
One blanket `[antibody-exempt]` covers 4,270 lines today. Each cask earns ONLY its
own marker: kernel-floor (`cascade_driver`, `dispatch*`, `policy_eval`,
`pm_dispatch`), pure-utility ("no domain" — the plumbing casks), or transitional
(`authz` reads the ACL bluebook — marker names the future migration). No cask
inherits the blanket. The blanket dies.

## Cask map (order = least-coupled -> most-coupled; each its own commit, green first)

### Wave 1 — pure leaves (free functions, zero state coupling, lowest risk)
1. `value_coercion.rs` — `as_bool/int/list/str`, `attr_value_from_str`
2. `json_convert.rs` — `value_to_json_string`, `value_from_json_str`,
   `json_to_value_recursive`, `json_obj_to_value_map`
3. `text_sim.rs` — `trigram_sim`, `fld`
4. `errors.rs` — error/result enums + their `Display`/`fmt` impls (types travel
   with their impls)

### Wave 2 — self-contained concerns
5. `boot.rs` — the 6 constructors
6. `authn.rs` — `authenticate_check`, `capture_auth`
7. `authz.rs` — `authorize_entry/check`, `service_gate`, `run_gate`

### Wave 3 — interpreter concerns (highest coupling, most care, last)
8.  `query_match.rs` — `where_matches`, `resolve_state_field` (or fold into `query.rs`)
9.  `ref_injection.rs` — `inject_refs`
10. `effect_outbox.rs` — `record_effect_outbound`, `publish_synthetic_event`
11. `cascade_record.rs` — `record_cascade_run`
12. `governance.rs` — `record_violation_internal`
13. `policy_eval.rs` — `evaluate_policy`, `evaluate_value_spec`
14. `pm_dispatch.rs` — `enumerate_pm_dispatches`, `fire_policy_cascade`,
    `pm_set_pairs`, `sweep_records` (split into 2 if >200)
15. `cascade_driver.rs` — `drain_outbound_to_quiescence`, `drain_policies`
    (THE kernel floor — keeps the one honest exemption)
16. `dispatch.rs` — bodies of `dispatch`, `dispatch_isolated`, `dispatch_deferred`
    (thin front-doors may stay in `mod.rs`)

## Per-cask recipe
1. Find the seam — the methods + private helpers forming ONE responsibility. No
   clean seam -> reconsider the boundary; do not force a line-cut.
2. Create `runtime/<cask>.rs` with a doc-header (name, intent, usage) + its honest
   marker (or none).
3. Move as an `impl Runtime` block (or free fns) with `use super::*`; cross-cask
   helpers become `pub(super)`.
4. `mod <cask>;` in `mod.rs`; delete the moved code.
5. `cargo build` clean + zero warnings; `cargo test --release` green — behavior is
   unchanged BY CONSTRUCTION, so any red is a botched move, fixed before commit.
6. Confirm cask <=200 code lines (doc header excluded) and `mod.rs` shrank by the
   same.
7. Commit — one cask, one commit, intent in the message.

## Definition of done
- `mod.rs` <= ~200 code lines: `mod`/`pub use` index + `Runtime` struct + thin
  front doors.
- NO file under `rust/src/runtime/` over 200 code lines (doc headers excluded).
- Blanket `[antibody-exempt]` gone; kernel-floor marker lives on
  `cascade_driver.rs` (+ the true-interpreter casks) alone.
- `cargo test --release` green throughout; every commit green; zero behavior
  change; zero new warnings.
- `core_runtime` LoC ~= unchanged (relocation); loc-ratchet delta ~0, no override.

## Constraints
- PURE MOVE — never change behavior in a move commit. A logic fix rides its own
  commit, before or after, never during.
- One cask per commit; each green before the next; never batch waves.
- Zero bugs: a red test is a botched extraction — fix at root, no skip.
- No file left >200 "temporarily". Genuinely-irreducible tight logic (likely only
  `cascade_driver`) is split along its internal seam or documented as the rare
  exception.
- Every cask gets a doc-comment header (the file-header standard).

## Sequencing
Run AFTER goal A. Waves 1 -> 3 in order; within a wave, any order.

## Non-goals
Bluebook migration (goal A); a separate `core.rs` (unneeded — `mod.rs` BECOMES the
thin residue, so after casking it is just the index — this answers "why is mod
separate from the runtime?": it isn't, once casked); refactoring dispatch/cascade
LOGIC (move it, do not touch it).

## Expected scorecard
`core_runtime` total: ~unchanged (relocation). File count: +~16 casks. Max file
size under `src/runtime/`: 4,270 -> ~200. This goal moves the GRANULARITY axis.

## Loop contract (what an autonomous runner reads to self-terminate)

### Work-list rule (DERIVED each iteration — not a fixed list)
The work-list = every file under `rust/src/runtime/` whose CODE-LINE count > 200,
MINUS the documented-irreducible list. `code_lines` = non-blank lines minus the
leading doc-comment header block — the SAME metric `bin/loc-ratchet` counts
(reuse it). Each iteration : pick the LARGEST file on the work-list, extract ONE
cask from it along a real seam. Empty work-list -> check the done predicate.

### Documented-irreducible list (the infinite-loop escape)
A file is exempt from the <=200 rule ONLY by an explicit marker in its own header :
`[cask-irreducible: <file> — <concrete reason the concern has NO internal seam>]`.
Expected occupant : at most `cascade_driver.rs`. The loop MAY PROPOSE a file for
this list (when it finds no clean seam after honest effort) but MUST HALT for
human confirmation — it can NEVER self-add the marker. This is what lets the loop
terminate on the one truly-irreducible cask without spinning, while keeping the
escape honest (a human signs each exemption).

### Per-unit done (one cask extraction)
ALL of :
  - `runtime/<cask>.rs` exists with a doc-comment header + its honest marker
    (kernel-floor / pure-utility / transitional / none).
  - the moved methods are GONE from the source file ; `mod <cask>;` added to
    `mod.rs`.
  - `cargo build` clean, ZERO warnings ; `cargo test --release` green.
  - the source file's `code_lines` dropped by ~the cask's size (proves a move,
    not a copy).
Commit the unit only when all hold. One cask, one commit. PURE MOVE — no behavior
change in the commit.

### Arc done predicate (one check, no prose)
```sh
# no non-exempt runtime file over 200 code lines, clean build, green
over=$(for f in $(find rust/src/runtime -name '*.rs'); do \
         grep -q 'cask-irreducible' "$f" && continue; \
         [ "$(code_lines "$f")" -gt 200 ] && echo "$f"; done)
[ -z "$over" ] \
  && (cd rust && ! cargo build --quiet 2>&1 | grep -q warning) \
  && (cd rust && cargo test --release --quiet)
```
(`code_lines` = loc-ratchet's non-blank-minus-doc-header counter.) All three :
no non-exempt file >200, zero warnings, tests green. True -> arc complete.

### Termination guards
- **False-done blocked** : the predicate ANDs "no non-exempt file >200" with
  green + warnings-clean. Green alone NEVER terminates — pure refactor, green
  throughout.
- **Infinite-loop blocked** : a file leaves the work-list only by dropping <=200
  OR gaining a human-approved `[cask-irreducible]` marker. The loop cannot
  self-exempt (so it cannot fake done) and cannot spin (on "no clean seam" it
  HALTS and proposes the marker).
- **Behavior-change guard** : each iteration is a PURE MOVE. If `cargo test` goes
  red, the extraction is botched — REVERT the move, do not commit, do not proceed.
  A logic fix is never smuggled into a move commit.
