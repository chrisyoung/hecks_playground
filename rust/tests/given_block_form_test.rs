//! Both spellings of a `given` block must parse to the same rule.
//!
//! Ruby takes a block either way — `do … end` and `{ … }` are the same
//! construct — so a parser that reads only one accepts a NARROWER language
//! than the DSL does. A bluebook is then valid Ruby and means something else
//! here.
//!
//! WHAT IT DID. `extract_block` looks for a brace. With none, the expression
//! fell back to the MESSAGE, so `given("the box is deep enough") do … end`
//! parsed to the rule `"the box is deep enough"` — a non-empty string compared
//! against nothing, permanently truthy. A rule that always passes is
//! indistinguishable from a rule that holds, which is why 144 corpus givens and
//! a green suite said nothing. Worse, the block's `end` then fell through to
//! the command's depth tracker and closed the command EARLY, so `emits` and
//! every later clause silently vanished with it.
//!
//! Zero of the corpus's 144 givens are written multi-line, which is exactly why
//! this survived : the construct was legal, unexercised, and wrong. The VO
//! invariant parser already carries the same scar in a comment ("Rust silently
//! read 2 of its 6 invariants while Ruby read all six").
//!
//! Usage : `cargo test --release --test given_block_form_test`

use storehouse::parser;

const BOTH_FORMS: &str = r##"Hecks.bluebook "GivenForms" do
  aggregate "Thing" do
    attribute :name, String
    attribute :size, Integer

    command "Register" do
      role "Operator"
      attribute :name, String
      attribute :size, Integer

      given("brace given") { !name.to_s.empty? }

      given("multiline given") do
        size > 0
      end

      given("inline do given") do size < 100 end

      emits "ThingRegistered"
    end
  end
end"##;

fn command(source: &str) -> storehouse::ir::Command {
    let domain = parser::parse(source);
    domain.aggregates[0].commands[0].clone()
}

#[test]
fn every_given_spelling_yields_its_predicate() {
    let cmd = command(BOTH_FORMS);
    let found: Vec<(String, String)> = cmd
        .givens
        .iter()
        .map(|g| (g.message.clone().unwrap_or_default(), g.expression.clone()))
        .collect();

    assert_eq!(
        found,
        vec![
            ("brace given".to_string(), "!name.to_s.empty?".to_string()),
            ("multiline given".to_string(), "size > 0".to_string()),
            ("inline do given".to_string(), "size < 100".to_string()),
        ],
        "each spelling must carry its own predicate, not its description"
    );
}

#[test]
fn a_given_never_parses_to_its_own_description() {
    // The specific shape of the bug, pinned on its own : a predicate equal to
    // its message is a rule comparing a string to nothing, and it can only
    // answer true.
    let cmd = command(BOTH_FORMS);
    for given in &cmd.givens {
        let message = given.message.clone().unwrap_or_default();
        assert_ne!(
            given.expression, message,
            "given {message:?} parsed to its own description — a rule that always passes"
        );
    }
}

#[test]
fn a_multiline_given_does_not_swallow_the_rest_of_the_command() {
    // The second half of the damage. The block's `end` decremented the
    // command's depth, so everything after the given — here `emits` — was never
    // parsed at all.
    let cmd = command(BOTH_FORMS);
    assert_eq!(cmd.givens.len(), 3, "all three givens survive the block");
    assert_eq!(
        cmd.emits.as_deref(),
        Some("ThingRegistered"),
        "emits must survive a multi-line given block above it"
    );
}
