# The given gate fails OPEN — three findings (2026-07-26)

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

## 1. Command attributes do not reach a given

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

Confirmed the same via the MCP door and the bare CLI, so it is not the
harness. Also confirmed against an attribute that does NOT share a name with
an aggregate field, so it is not name shadowing.

Why it fails open rather than loudly : an unresolvable bare name falls
through to `Value::Str(expr)` (`interp_expr.rs:39`), so `word` resolves to
the literal string `"word"` — truthy, non-empty, and unequal to anything
the author meant.

**Blast radius unknown.** Any given reading a command attribute rather than
stored state is currently not guarding. Worth a corpus grep before deciding
severity.

## 2. A bare non-Bool given silently passes

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
