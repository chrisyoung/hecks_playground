# The given gate fails OPEN — two findings remain (2026-07-26)

> STATUS (updated 2026-07-26 evening) : findings 0 and 1 are FIXED. Finding 1
> turned out not to be a given bug at all — the single-file CLI branch was
> discarding its whole argv payload. Findings 2 and 3 are unchanged and still
> open ; both are deliberate fails-open defaults that need a decision rather
> than a patch, and finding 2 explicitly should NOT be flipped blind.

Found while writing hecksagain's `grammar/bluebook.bluebook`, by describing
the language in itself and watching which sentences the runtime could not
evaluate. One of the four is already fixed and committed on this branch
(the negation branch, `interp_givens.rs`). The other three are open, and
they share a shape worth naming :

**every one of them fails OPEN.** An expression the interpreter cannot
evaluate does not refuse the command — it resolves to something benign and
lets the command through. A given that cannot be read is a given that does
not guard, and nothing in the suite notices, because a rule that always
passes looks exactly like a rule that is satisfied.

Reproduction probes live at `/tmp/negproof/` (`negproof.bluebook`,
`matrix.bluebook`) — throwaway, re-create if gone.

---

## 0. FIXED — negation swallowed by the `.empty?` suffix match

`!value.empty?` matched the `.empty?` branch, which stripped the suffix off
`!value` rather than `value`. 20+ corpus rules reading "must not be blank"
never fired. Fixed on this branch ; 1050 cargo tests green, 152/152
`.behaviors` green. Recorded here only so the four are read together.

---

## 1. FIXED (2026-07-26) — command attributes do not reach a given

    command "WordEquals" do
      attribute :word, String
      given("word is Hi") { word == "Hi" }
    end

    storehouse matrix.bluebook Matrix::Probe.WordEquals word=Hi
    -> GivenFailed

The IR is correct — `describe_aggregate` shows `word` as a declared command
attribute of type String. `command_dispatch.rs:371` hands `attrs` to
`check_givens`. So the break is upstream, in argv -> attrs binding, and I
did not chase it into `rust/cli/src/main.rs` (7k lines, several `split_once('=')`
sites) rather than guess.

Why it fails open rather than loudly : an unresolvable bare name falls
through to `Value::Str(expr)` (`interp_expr.rs:39`), so `word` resolves to
the literal string `"word"` — truthy, non-empty, and unequal to anything
the author meant.

**The diagnosis was right and the site is found.** `cli/src/main.rs` — the
single-FILE dispatch branch parsed its `k=v` pairs into `attrs` and then
called `rt.dispatch(cmd_name, HashMap::new())`, discarding every one. The
payload never arrived, so `word` was genuinely undeclared at evaluation.
Not a given bug at all — a transport bug wearing a given bug's clothes.

Blast radius, now known and bounded : the sibling DIRECTORY branch
(`dispatch_hecksagon`) always passed attrs through, so the MCP door and
`storehouse <root> …` were never affected. Only `storehouse <file.bluebook>
…` dropped its payload — the form no script in the repo uses, and the one
README.md:29 teaches first. No corpus given was silently unguarded ; the
corpus dispatches through the door.

The note above that this reproduced "via the MCP door" is the one part that
misleads : `storehouse__dispatch` accepts a single bluebook FILE as its
`aggregates_dir`, and pointed at a file it takes this same broken branch.
The door was fine ; the path through it was not.

This also silently defeated the `%w[]` literal-set work committed earlier on
this branch — that interpreter fix was correct and complete all along, but
its live repro ran through this path, so an out-of-set value was never
refused and the work looked unfinished. Two investigations, one cause.

Pinned by `rust/tests/single_file_payload_argv_test.rs`, which spawns the
real binary (every other gate test boots `Runtime` directly and so enters
BELOW the transport, which is why none of them could arbitrate). Verified
to go RED when the bug is reinstated — per the meta-finding below.

## 1b. FIXED (2026-07-28) — the permissive default also fails CLOSED

Found by auditing the bare arm's population before flipping it (finding 2's
prescribed "log every `_ => true` hit first"). The audit walks 334 corpus
bluebooks, classifies every `given` / `holds_when` clause by which branch of
`evaluate_given` claims it, and reports the leaves that reach the bare arm.
Ten. Six are banking's rich-VO predicates (`balance.covers?(amount)`), which
resolve to Bool correctly. The other four were dead :

    Feature.PlanAdditions      given { !title.nil? && !title.empty? }
    Feature.VerifyAdditions    given { !additions.nil? && !additions.empty? }
    Notification.DismissAlert  given { !alerts.nil? && alerts.size > alert_index }
    Search.SearchDomain        given { !query_text.strip.empty? }

Neither `.nil?` nor `.strip` was implemented. `title.nil?` fell to the bare
arm, PASSED by the permissive default — and then negated to a permanent
**false**. `query_text.strip` was looked up as a flat field name, missed,
reported size 0, so `.empty?` answered true and `!` made it false again. All
four commands were UN-DISPATCHABLE, with any payload. Verified end-to-end
against the real binary before the fix and after.

**This is the finding that changes the shape of finding 2.** The permissive
default does not only let unreadable rules through ; under a leading `!` — how
the corpus writes most presence checks — it silently refuses everything. Both
verdicts are the interpreter's, not the author's. "Fails open" was half the
diagnosis.

Fixed : `.strip` and a recursing `.size` receiver in `interp_expr.rs`, `.nil?`
in `interp_predicate.rs`. Pinned by `rust/tests/nil_strip_predicate_test.rs`,
which exercises every rule in BOTH directions.

The audit itself is now a GATE — `rust/tests/given_bare_branch_audit_test.rs`
refuses any bare-arm clause that does not name a declared `Boolean` derivation,
the one shape the arm can actually judge. Corpus today : 6 judgeable, 0 not.
A new `given { thing.whatever? }` fails at test time instead of at dispatch.

## 2. FIXED (2026-07-28) — a bare non-Bool given silently passes

> Landed the way this section asked : audit first, read the list, then flip. The
> audit found 10 bare-arm clauses across 334 bluebooks — 6 legitimate Boolean
> derivations and the 4 dead commands of finding 1b. With those repaired the
> corpus had ZERO unjudgeable givens, so the flip surfaced nothing and broke
> nothing : full workspace sweep green, 152 test binaries.
>
> The gate now has THREE outcomes, not two. `judge` returns
> `Result<bool, Unjudgeable>` and propagates through `||`, `&&` and `!` ; the
> bare arm returns `Err` instead of `true` ; `check_givens` reports
> `RuntimeError::UnjudgeableGiven { expression, clause }`, naming the leaf it
> could not read. A defect in the rule is no longer spelled as a refusal of the
> payload — nothing the caller sends can satisfy a predicate nobody can read.
>
> Both clauses of `&&` / `||` are judged even when the first decides the
> verdict : an unreadable clause is a defect wherever it sits, and a
> short-circuit would hide exactly the half nobody exercises. A given has no
> side effects, so there is nothing to short-circuit FOR.
>
> `evaluate_given` survives as a total wrapper (`judge(...).unwrap_or(false)`)
> for the three callers with no error channel — aggregate invariants, the
> payload gate, and a derivation body inside `resolve_expr`. Unreadable is FALSE
> for them : refused loudly rather than passed silently.
>
> Pinned by `rust/tests/unjudgeable_given_test.rs` — a typo'd bare name and an
> unimplemented method are both UnjudgeableGiven ; a readable rule still refuses
> as a plain GivenFailed carrying the author's message, and still admits when
> satisfied. The flip bites, and it does not over-refuse.
>
> HONEST LIMIT, and it is finding 3's : a typo INSIDE a comparison
> (`given { conut > 0 }`) is still not caught. `resolve_expr` spells an unknown
> bare name back as a Str, so the comparison reads `"conut" > 0`, coerces to 0,
> and answers false — a silent refusal wearing the author's message. Only BARE
> leaves reach the arm that was flipped. The self-spelling fallback is the next
> flip, and it is a bigger one.
>
> `interp_predicate.rs` crossed 200 lines on the way, so the Ruby idioms
> (`.nil?`, `.any?`, `.empty?`, `.include?`) moved to `interp_idioms.rs` — they
> share one rule (suffix matching, hence the leading-`!` guard) and now say so.

ORIGINAL TEXT :

`evaluate_given` ends :

    match resolve_expr(expr, state, attrs, ctx) {
        Value::Bool(b) => b,
        _ => true,
    }

The comment calls this "the historical permissive default". It means any
bare given whose expression does not resolve to a Bool is a no-op — including
every typo, every renamed attribute, and every expression using an operator
the floor does not implement. This is what made `given { word }` appear to
pass in the probe above ; it was not truthiness, it was the default.

hecksagain refuses here instead : an unresolvable name RAISES, on the
principle that a predicate that cannot be evaluated is a defect, not a false.

Changing `_ => true` to a refusal is the correct direction and is NOT safe to
do blind — it will surface every currently-silent given at once. Suggest
landing it behind a one-run audit : log every `_ => true` hit across the
corpus first, read the list, then flip.

## 3. `.size` reads a flat map and answers 0 for anything else

    if let Some(field) = expr.strip_suffix(".size") {
        let val = attrs.get(field).cloned()
            .unwrap_or_else(|| state.get(field).clone());
        return match val {
            Value::List(v) => Value::Int(v.len() as i64),
            Value::Str(s)  => Value::Int(s.len() as i64),
            _ => Value::Int(0),
        };
    }

Two problems. It does not recurse through `resolve_expr`, so a dotted path
(`price.cents.size`) cannot work. And `_ => Value::Int(0)` means an
unresolvable receiver reports size zero rather than refusing — so
`x.size == 0` passes for a field that does not exist, and `.empty?`, which is
implemented as a rewrite to `.size == 0`, inherits it.

hecksagain computes `.empty?` directly and refuses a receiver with no size.
That divergence is now a real difference between the two runtimes on the same
text, which is exactly the kind of thing the parity harness exists to refuse
— worth closing deliberately in one direction.

---

## The meta-finding

There were NO tests for `.empty?` / `.any?` anywhere in the Rust suite. The
negation bug survived 20+ corpus usages and 1046 green tests because a rule
that always passes is indistinguishable from a rule that holds.

The cheap structural defence is a **fails-open audit** : a test that asserts
every admitted operator has both a passing AND a refusing example. A rule
that cannot be made to fail is not a rule, and today nothing checks that.
