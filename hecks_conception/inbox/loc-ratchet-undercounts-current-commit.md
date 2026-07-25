# loc-ratchet undercounts the CURRENT commit's growth (a shrink concern can grow ungated)

## Symptom (observed 2026-07-25)

The framework-substrate de-embed commit `e336bd9dd` grew `core_runtime` by **+63**
non-blank LoC (a `shrink` concern). Its commit-time ratchet reported
`core_runtime shrink base=40459 head=40459 delta=0` and PASSED — the growth
landed on the branch **without** the `[loc-ratchet-override]` the concern requires.
The +63 only surfaced on the NEXT ratchet run (a follow-up `Cargo.lock` commit),
which then blocked with `base=40459 head=40522 delta=63`.

Net: a gate whose whole job is to fence kernel growth let a real +63 through the
commit that introduced it. A gate that under-counts is worse than no gate — it
reads green while the thing it guards drifts.

## Root cause (bin/loc-ratchet)

`head = counts_at(concerns, "HEAD")` (line ~249). `counts_at` reads each concern
file's blob at a **git treeish** via `git ls-tree -r <treeish>` + `git show
<treeish>:<path>`. When loc-ratchet runs as the commit-time hook, the new commit
does not exist yet, so `HEAD` resolves to the **PARENT** tree — which does NOT
contain the staged change. So the current commit's own contribution is invisible
to the count : `head` == parent == (often) `base`, giving `delta=0`.

The figure it SHOULD count at commit time is the **staged index** (the tree about
to become the commit), not `HEAD`.

## Why cold-log (+217) WAS caught — the nuance the fix must reconcile

The cold-log commit's +217 DID block at commit time, which means the count saw
staged content in that case. So the mechanism is not a clean "always reads
parent" — the hook's invocation point (pre-commit vs commit-msg vs pre-push) and
whether a prior branch commit already carried the growth both matter. The fix
must be verified against BOTH cases : a fresh single-commit growth (must block)
and a follow-up commit on a branch that already grew (must stay gated).

## Fix direction

Count the **staged index tree** for the head figure when gating a commit, not the
`HEAD` ref. `git write-tree` materialises the index as a tree object ;
`counts_at(concerns, <index-tree-sha>)` then counts exactly what is about to be
committed. At pre-push time `HEAD` is already correct (the commits exist), so the
correct treeish is context-dependent :
  - commit-time gate (has a `--message-file`) -> count `git write-tree` of the index
  - pre-push / manual -> count `HEAD` (current behaviour)

Select the head treeish off the same signal the override-scope logic already uses
(`--message-file` present == gating ONE commit).

## Verification (before trusting the fix)

1. Stage a +N non-blank-LoC change to a `shrink` concern (no override) and
   `git commit` — the commit-time gate MUST block with `delta=N` (today: passes at 0).
2. Add the `[loc-ratchet-override: …]` marker — the SAME commit MUST pass.
3. A follow-up commit on the branch with no new growth MUST still pass (no
   double-count of the already-authorised growth).
4. `bin/loc-ratchet --base origin/main --verbose` (manual, HEAD) unchanged.

## Severity

High for the discipline : this is the gate that keeps kernel LoC honest. Every
commit since the bug could have grown a shrink concern by its own contribution
ungated. Worth an audit of recent `shrink`-concern deltas vs their commits once
fixed.
