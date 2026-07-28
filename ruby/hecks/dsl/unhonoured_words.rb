module Hecks
  module DSL

    # Hecks::DSL::UnhonouredWords
    #
    # Words a bluebook may say that NEITHER runtime honours — declared by the
    # author, acted on by no one. Gathered in one place so the Ruby DSL reads
    # past them exactly as the Rust parser does, and so the list of promises
    # the language is currently breaking is one file rather than a rumour.
    #
    # Rust line-scans a bluebook ; Ruby EVALUATES it. So a word Rust merely
    # skips is, on this side, a NoMethodError that makes the whole domain
    # unreadable — not a missing field, a missing FILE. That asymmetry hid 25
    # unloadable bluebooks (2026-07-28 sweep), and the parity harness could not
    # see any of them : it diffs the IR of files BOTH sides can read.
    #
    # Each word here is consumed rather than interpreted because that is what
    # Rust does with it TODAY, verified against `storehouse dump`. This module
    # is a parity floor, not an endorsement — every word in it is a declaration
    # the author wrote and neither runtime honours, which is worth fixing at
    # the domain level. Giving one of them meaning means giving it meaning in
    # BOTH parsers, on purpose, with the goldens updated.
    #
    #   rule "must be non-empty" do   — i246 lifts rules into first-class IR ;
    #     requires { !value.empty? }    until then consume_rule_block walks the
    #   end                             whole block (rust parse_blocks.rs:2121)
    #
    #   requires "status" => "pending" — a precondition inside a command. Rust
    #                                    recognises `requires` ONLY inside a
    #                                    rule block, so this form gates nothing.
    #
    #   delivery :actor               — sprint-14 delivery mode. No parser
    #                                    surface on either side.
    #
    module UnhonouredWords
      # `rule "name" do ... end` — the block is NOT evaluated. Rust does not
      # read inside it either (beyond validating a `requires { }` body shape),
      # and evaluating it here would call words that exist in no builder.
      def rule(*_args, **_kwargs, &_block)
        # consumed — see module header (i246)
      end

      # `requires "status" => "pending"` / `requires { predicate }`.
      def requires(*_args, **_kwargs, &_block)
        # consumed — see module header
      end

      # `delivery :actor` / `delivery :sync`.
      def delivery(*_args, **_kwargs, &_block)
        # consumed — see module header
      end

      # `expects "phrase resolves in the lexicon" do ... end` and
      # `guarantees "phrase_count reflects the new total" do ... end` — the
      # pre/post-condition pair the storehouse chapters are written with.
      # Neither becomes a `given` in the Rust IR (verified : Dispatch.Route
      # dumps `givens: []` while carrying an `expects` block), so a contract
      # the chapters state in prose is enforced by nothing. Consumed here to
      # match ; lifting them into real preconditions is a both-runtimes change.
      def expects(*_args, **_kwargs, &_block)
        # consumed — see module header
      end

      def guarantees(*_args, **_kwargs, &_block)
        # consumed — see module header
      end
    end
  end
end
