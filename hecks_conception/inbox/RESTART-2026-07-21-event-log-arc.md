# CLOSED — the event-log arc (governable, lineage-rich, authoritative, trusted)

All five stages landed 2026-07-22, `main` green throughout (134 test binaries,
zero failures, zero warnings). The plan this came from lives at
`~/.claude/plans/this-is-hard-for-concurrent-blum.md`.

## The five stages

1. **Governability** — the log records WHO acted under WHICH policy, stamps the
   subject bluebook's version + the runtime build, and gives every business FLOW
   one correlation id (`2ddfc2c88`, `75dad37e8`, `af70166a5`, `e3dfcc505`).
2. **Lineage** — `CausationTrace` walks backward ; `ConsequenceTree` fans out
   forward over a reverse index, depth-annotated and deterministic (`cf6c106c9`).
3. **Snapshots** — threshold capture (never a timer — the bluebook says so twice),
   the capture IS the read, so one fold serves both (`8f5588c9f`).
4. **The read side** — `load = Snapshot@watermark + fold_forward(tail)`. State is
   DERIVED from the log. Proven cold: delete the current-state store and a fresh
   `storehouse` process still answers (`cbbdd132d`).
5. **Trust** — per-shard hash chain + `ChainIntact`, distinguishing CONTENT edits
   from LINK breaks (`48ed26686`) ; and derivability as a STANDING invariant — a
   `Verification` singleton whose verdict is queryable domain state, fired by a
   60s Driver, sharing ONE gauge with `storehouse verify-projection`
   (`c27629dfa`).

Plus `202478c78` — the loc-ratchet override authorises ONE commit, not a whole
branch (it had been waving through +568 lines of core_runtime on a marker written
ten commits earlier).

## Five defects the proofs uncovered

Every one was invisible to a passing test and found by dumping real rows. This is
the lesson worth carrying forward: **a green test on event-sourced code is a
hypothesis until you look at the log.**

- The log DROPPED emitted events — an empty-delta early return discarded the
  first-class event-row for any command that changed no state.
- Causation pointed at a bookkeeping DELTA row, not the domain event, so
  `CausationTrace` had been answering the wrong question since `2ddfc2c88`.
- Lineage rows rendered value objects as `"{N fields}"` — the same lossiness
  `6b7ed1312` fixed inside the log, reintroduced by the query reporting it.
- The read side was wired into ONE of two boot doors, so every cold CLI
  invocation still read the store alone. Only the cold smoke caught it.
- The hash chain never reached the log (attribute not declared on `Append`) and
  its digest was map-iteration-order dependent — it would have cried wolf.

## What remains (named, not hidden)

- **A cryptographic digest + an external anchor** if the chain is ever to resist a
  determined rewriter rather than a careless one. FNV-1a-64 today, deliberately:
  a crypto hash with a still-local head only LOOKS stronger.
- **Global sequence.** `sequence` is per-PROCESS until the merge assigns global
  order, so the fold orders by `recorded_at` with sequence as tiebreak, and a
  snapshot watermark is only meaningful against its own process's ordering.
- Deferred by the original plan and still deferred: schema-evolution upcasting
  (the version stamp is now in place to support it), retention/forget-me,
  optimistic concurrency, the floor→adapter finish for the IO seams
  (`persistence_resolution.rs` still hardcodes `"AppendLog" =>`), per-event
  reducers.

## Locked decisions (do NOT re-litigate)

- The log write STAYS a synchronous kernel writer, not a declared policy. An async
  policy fires in the reaction phase, so a crash between the state save and the
  append loses the event. Stage 4 additionally dissolves that window from the read
  side.
- The log stays in EventSourcing — no `→ Log` rename.
- `event_sourcing` is NOT a specializer target. "Projection from bluebook" here
  means INTERPRETATION, not codegen ; hand-written kernel floor is correct for it.
- Both `event_sourcing.bluebook` copies stay byte-identical (substrate_parity_test
  guards it) — edit `rust/resources/`, then `cp` to the conception copy.
