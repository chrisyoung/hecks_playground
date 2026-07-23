# RESOLVED — the cold-log gap was a SPLIT-BRAIN (2026-07-23)

Fixed on `fix/cold-log-split-brain`. 982 tests pass, 0 fail, clippy clean.
Kept as the record, because the first diagnosis in this file was WRONG and the
way it was wrong is the lesson.

## The original diagnosis was wrong

This note used to say the cold write "never reaches durable storage" and is
"lost on process exit". Both false. Two measurements settled it:

- `storehouse heki count <event.heki>` before/after a cold dispatch: **5878 ->
  5880**. The rows were on disk all along, stamped with the right
  `bluebook_version`.
- `stat` on the two candidates: `event.heki` grew 431970 -> 433054 bytes, while
  `event.log` sat frozen at 166248980 bytes, mtime 2026-07-06, untouched by any
  dispatch.

Nothing was lost. The write went to **the wrong store**.

## What it actually was

| | repository | substrate |
|---|---|---|
| **writer** | the framework COLLABORATOR's `Event` repo — `boot_with_data_dir` attaches no hecksagons, so it fell through to the implicit heki default | `event.heki` |
| **reader** | an AppendLog-bound `Event` repo | `event.log` + shards |

Two `Event` repositories over two substrates, each succeeding, neither aware of
the other. That is why cold `state` answered while cold `Event.Replay` returned
`[]` for the SAME command — they were never two views of one store. And why all
253 shards were 0 bytes: `append_log_save` is `Backend::AppendLog`-only, so
nothing ever wrote what `event_shard::reserve` had opened.

`record_event_append`'s own comment already asserted the correct binding —
*"Append's save routes through the AppendLog adapter to THIS process's shard"* —
and was simply untrue.

In-process it LOOKED correct by coincidence: both repos were heki over the SAME
file, so the reader's lazy hydrate picked up the writer's rows.

## The fix : the Event Log's substrate is an INVARIANT, bound in ONE place

- `runtime/mod.rs` — `bind_event_log_to_appendlog()` runs inside
  `boot_with_data_dir`, the single path every runtime boots through, so the
  collaborator and any runtime carrying the EventSourcing substrate cannot
  disagree. Done programmatically rather than by compiling in the hecksagon:
  the lazy collaborator boot exists so ~124 test binaries don't pay a parse;
  `resolve_bindings` type-checks against `.adapter`/`.family` files that are not
  in `rust/resources`; and the Log's backend is a kernel invariant, not a
  per-deployment choice.
- `event_log.rs` — `unconsolidated_tail_states()`, ONE shared read-only
  shard-tail reader (private checkpoint copy, one `merge_pass`, discard).
  `unconsolidated_log_tail` now delegates to it; the two copies had ALREADY
  drifted (the old one gave every tail record an empty id).
- `lazy_repository.rs` — AppendLog seed = global + tail, and the query PUSHDOWN
  gets the tail too. The pushdown reads `event.log` directly and bypasses the
  repository cell, so seeding alone left `Replay` blind. The tail goes in
  unfiltered, which is safe by construction: the oracle re-applies every clause,
  so a prefilter can only narrow.

## Two latent defects this exposed — both real, both fixed

**1. The shard sink was one per PROCESS, not per realm.** `event_shard` held a
single `OnceLock` rooted at whichever `shard_dir` called FIRST; every later
append went there regardless of which realm produced it, silently, because the
write still succeeded. `storehouse serve --multi` hosts several realms in one
process, so this filed one realm's events under another realm's shard. Now keyed
by directory. **This was the root cause of 7 of the 8 failing test targets** — a
test binary whose ten tests each use their own temp dir is just the cheapest
reproduction of a multi-realm process.

**2. `maybe_capture_snapshot` was O(whole-log) per dispatch.** It computed the
head sequence as `max(sequence)` over `all_qualified(EventSourcing, "Event")` —
materialising and folding the ENTIRE Log on EVERY command to learn one integer.
Replaced with `log_head_sequence()`: the `.seq` sidecar (O(1)) maxed with the
unconsolidated tail (O(tail)). Identical value, two cheap sources.

## Cold dispatch, against the real 166MB event.log

| | |
|---|---|
| `main` | 0.37s — fast only because it read the small WRONG store |
| split-brain closed, no perf work | 11-12s — the O(log) head scan surfacing |
| final | **0.090s** |

Faster than `main` ever was: the head scan was always wasteful, it was just
reading a smaller wrong file.

## Verified
- a cold `Event.replay` returns events written by SEPARATE earlier cold processes
- `event.heki` no longer grows on dispatch
- shards carry real bytes (1493, not 0)
- read-only invocations create no shard files
- `backend_map_phase_a_test` now PINS the invariant (Event is AppendLog, every
  other repo heki) instead of loosening its `all()` — it still catches a silent
  backend change, and now also catches the Log LOSING its binding.

## STILL OPEN — a data question someone must decide

`event_sourcing/event.heki` holds **~5900 event rows** written to the wrong store
by the split-brain — real audit trail (InboxPoller, ShellTool, FileTool, Inbox,
AgentMessage, Account). New events now go to shards -> `event.log`, so those rows
are orphaned history. **Migrate them into `event.log`, or knowingly abandon
them.** Do not let it be decided by accident.

Also worth a look: `~/Library/Application Support/Hecks/hecks/shards/` has ~380
shard files, most of them 0-byte orphans from the broken era, and `event.log` has
not been consolidated since 2026-07-06. Consolidation has not been running.

## Deferred (unchanged, from the arc)
crypto digest + external anchor ; **global sequence** — note this is not merely
cosmetic: the consolidated log carries GLOBAL sequences while the shard tail
carries PER-PROCESS ones, so `sequence.value > watermark` mixes two numbering
spaces and a fresh tail record can sort below a snapshot watermark ;
schema-evolution upcasting ; retention/forget-me ; the floor->adapter finish for
IO seams (`persistence_resolution.rs` hardcodes `"AppendLog" =>`).

## The meta-lesson, now earned twice

The diagnosis in this file was written by reading source and was wrong in its
central claim. It took `heki count` before/after and a `stat` on two files to see
the truth. On event-sourced code, **look at the bytes** — and check you are
looking in the directory the runtime actually writes to. `std::env::temp_dir()`
on macOS is `$TMPDIR`, not `/tmp`, and the heki store is realm-anchored to
app-support, not to the `data/` dir you passed in. I looked in the wrong place
twice.

And: **`cargo build --release` in `rust/` rebuilds only the lib.** The CLI is a
separate crate — `cargo build --release -p storehouse-cli`. A stale binary cost a
full wrong measurement cycle here, the same trap that bit the i629 session.

## Repro (kept, works)
`/tmp/coldlog_repro` — framework substrate + a tiny event_sourced Vault.
```
cd /tmp/coldlog_repro
SH=~/Projects/hecks/rust/target/release/storehouse
$SH aggregates Vault::Vault.OpenVault                                  # cold write
$SH query aggregates EventSourcing::Event.replay aggregate_name=Vault  # cold read
```
Store resolves to `~/Library/Application Support/Hecks/hecks/`, NOT the realm dir.
