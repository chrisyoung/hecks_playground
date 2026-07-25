# Goal A: Retire the bespoke adapter resolvers — the runtime's IO protocol becomes bluebook

> Axis: DOMAIN (Rust -> bluebook). Sibling goal: goal-b-cask-runtime.md (granularity).
> Run A BEFORE B (A deletes ~645 lines, so B has fewer casks to make).

## Objective
Remove the four remaining hand-written per-adapter resolvers from
`rust/src/runtime/mod.rs` — `resolve_claude_tool_adapters`,
`resolve_llm_adapters_with_excluded`, `resolve_mcp_adapters`,
`resolve_compute_adapters_with_excluded` — replacing each with a thin primitive
leaf plus a bluebook-declared `fire -> run -> cascade` protocol, exactly as
`resolve_exec_adapters` was already retired in favour of `resolve_primitive_spawn`
+ `Primitive::Process.Spawn`. **Done = zero bespoke `resolve_*_adapters` remain**;
every IO adapter dispatches through one generic primitive path driven by ordinary
bluebook policy/cascade.

## Why
These are ~645 lines of the SAME protocol (guard -> match binding by kind/trigger
-> build attrs -> call the type dispatcher -> cascade `result_into`) copied around
a different dispatcher call. The protocol is domain; only the dispatcher syscall
leaf is irreducible Rust. This is bluebook-first made literal, and it shrinks
`core_runtime` (a shrink concern) by ~485 lines of Rust BECOMING domain — not
relocation.

## Proven template (copy it)
`resolve_exec_adapters` -> retired. `resolve_primitive_spawn` is the generic leaf
(read attrs -> `exec_dispatcher::dispatch` -> `dispatch_cascade(result_into)`); the
wrapper is a `Primitive::Process.Spawn` command + bluebook policy/cascade. Each
migration repeats this shape.

## Scope, smallest-first (each its own commit)
1. `compute` (~95) — clearest 1:1 with the template
2. `mcp` (~181)
3. `claude_tool` (~188)
4. `llm` (~181)

## Per-migration recipe
1. **Bluebook first** — extend `Tools.bluebook` (or the adapter's family) so this
   adapter's fire->run->cascade is declared policy/cascade, keyed on the
   triggering command, with `result_into` as the cascade target.
2. **Thin leaf** — in the `<x>_dispatcher` module: read attrs -> call the
   dispatcher -> return the result the cascade records. The IO syscall is the only
   surviving Rust.
3. **Delete** the bespoke `resolve_<x>_adapters` from `mod.rs` and its call site in
   the dispatch hook — cleanly, no shim.
4. **Prove end-to-end** — a runtime test: dispatch the triggering command; assert
   the dispatcher fired AND the `result_into` cascade landed on the same
   invocation record. Extend behaviors for the newly-declared protocol.
5. **Verify** — parity where a golden exists; full suite green; confirm
   `core_runtime` DROPPED by the resolver's line count (loc-ratchet moves the
   right way — a shrink, NO override).

## Definition of done
- All four `resolve_*_adapters` gone; the dispatch hook calls one generic
  primitive path.
- Each adapter's protocol lives in a bluebook, not Rust.
- `core_runtime` net-shrinks ~485 lines vs `origin/main`, NO loc-ratchet override
  (genuine shrink).
- `cargo test --release` green; each migration has a cascade-fires-and-lands test;
  behaviors green.

## Constraints
- Bluebook-first: conceive the declared protocol BEFORE deleting the Rust — never a
  resolver removed with nothing to replace it.
- One resolver at a time, each independently provable and green before the next.
  Never batch.
- Zero bugs: a failing cascade test halts the arc until fixed at root — no SKIP, no
  bypass.
- No backward compat: delete cleanly, no dual path.

## Non-goals
The interpreter core (`dispatch`, `drain_*`, given-enforcement — the irreducible
floor); the `mod.rs -> core.rs` structural split; the value-helper extraction; the
boot builder. (Those are goal-b or later.)

## Expected scorecard
`core_runtime` total: **-~485** (Rust became domain). File count: +~4 tiny leaves
(relocated into dispatcher modules). This goal moves the DOMAIN axis; file-size of
mod.rs only drops ~645 (the resolvers leave), the rest is goal-b.

## Loop contract (what an autonomous runner reads to self-terminate)

### Work-list rule
Fixed set, smallest-first : `[compute, mcp, claude_tool, llm]`. Each iteration,
pick the first adapter whose `resolve_<x>_adapters` still exists in
`rust/src/runtime/mod.rs`. Empty work-list -> check the done predicate.

### Per-unit done (one adapter X)
ALL of :
  - `fn resolve_<X>_adapters` is gone from `mod.rs` AND its call site in the
    dispatch hook is gone.
  - `Tools.bluebook` (or X's family) declares X's fire->run->cascade as
    policy/cascade keyed on the triggering command.
  - a test asserts the X dispatcher fired AND the `result_into` cascade landed on
    the same invocation record (cascade-fires-and-lands).
  - `cargo test --release` green.
Commit the unit only when all hold. One adapter, one commit.

### Arc done predicate (one check, no prose)
```sh
! grep -qE 'fn resolve_(claude_tool|llm|mcp|compute)_adapters' rust/src/runtime/mod.rs \
  && (cd rust && cargo test --release --quiet) \
  && bin/loc-ratchet --base origin/main | grep -qE 'core_runtime.*delta= *-'
```
All three : no bespoke resolver remains, tests green, `core_runtime` delta is
NEGATIVE (it shrank — proves Rust dissolved, not moved). True -> arc complete.

### Termination guards
- **False-done blocked** : the predicate ANDs the structural grep with green.
  Green alone NEVER terminates — this is a pure refactor, green throughout.
- **Shrink required** : `delta= -` guards against a resolver being relocated
  instead of dissolved ; if `core_runtime` did not shrink, it is not done.
- **Stall halt** : if an iteration cannot reach per-unit done (dispatcher won't
  fire, cascade won't land, a golden breaks), HALT and escalate to a human.
  Never delete a resolver without its bluebook replacement proven ; never skip a
  red test ; never bypass the ratchet.
