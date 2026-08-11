module Hecksagain
  module Bluebook
    module DSL
      class PolicyBuilder
        def initialize(name)
          @name = name
        end

        # Vendored alias, not (yet) upstream hecksagain -- see
        # CommandBuilder's identical `description`/`goal` alias comment.
        def description(value) = nil

        def on(event_name) = @on_event = event_name.to_s

        def trigger(command_name) = @trigger_command = command_name.to_s

        def across(domain_name) = @target_domain = domain_name.to_s

        # `where field: value` -- vendored addition, not (yet) upstream
        # hecksagain (migration plan task 8): a conditional policy
        # trigger, gating whether the policy fires on the triggering
        # EVENT'S OWN PAYLOAD (not a stored-record query the way an
        # aggregate query's `where` is) -- deciderate's own comment
        # names it exactly : "the where-guard IS the 'round complete'
        # condition -- the saga," AdvanceRound firing only when
        # `games_remaining: 0` rides the ResolutionRecorded event. See
        # Runtime::PolicyInterpreter#policies_for's own comment for the
        # evaluation side.
        def where(**conditions)
          (@wheres ||= {}).merge!(conditions.transform_keys(&:to_sym))
        end

        # `for_each from: "Aggregate.query_name", where: { field:
        # from_event(:x) }` -- vendored addition, not (yet) upstream
        # hecksagain (migration plan task 8): a policy-level fan-out,
        # the SAME shape as `dispatch ..., for_each:` already built for
        # saga handlers, except a policy has no saga instance to source
        # from -- every `where:` value resolves against the triggering
        # EVENT's payload only. deciderate's own SettleVotesOnPop names
        # it exactly : "a pop settles the bubble's live votes" -- one
        # Vote.Settle per row `Vote.ForBubble` returns, not one dispatch
        # total. See Runtime::PolicyInterpreter#deliver's own comment
        # for the evaluation side.
        def for_each(from:, where: {})
          @for_each = { from: from.to_s, where: where }
        end

        # `from_event(:field)` -- vendored addition, not (yet) upstream
        # hecksagain (migration plan task 8): the SAME trivial sugar as
        # HandlerBuilder's own `from_event` (a PM handler's sibling) --
        # returns the bare Symbol, relying on `for_each`'s `where:` hash
        # resolving a Symbol value against the triggering event's
        # payload at delivery time, exactly like a `with:` spec already
        # does for `dispatch`.
        def from_event(field, default: nil) = field

        # Vendored addition, not (yet) upstream hecksagain -- see
        # IR::Policy#with_literals's own comment.
        def with(key, value)
          (@with_literals ||= {})[key.to_s] = value
        end

        # Vendored addition, not (yet) upstream hecksagain: `map target:
        # :event_field` renames/selects an event-payload field for the
        # triggered command's argument -- structurally accepted here
        # (folded into the same with_literals merge, sourced from a
        # `:event_field` symbol marker the interpreter could resolve
        # against the actual event payload) so the corpus boots; exact
        # select-vs-merge runtime semantics need review before this is
        # trusted for a live dispatch. TODO upstream via bin/evolve
        # (migration plan task 7); TODO verify runtime semantics before
        # any file relying on `map` goes live.
        def map(**pairs)
          (@with_literals ||= {}).merge!(pairs.transform_keys(&:to_s))
        end

        # Vendored no-op stub, not (yet) upstream hecksagain (migration
        # plan task 8): `condition { |event| event.field > x }` -- found
        # 15 times across hecks_nursury, always a comparison/boolean
        # expression over the triggering event's own payload via a
        # block PARAMETER (`event.severity == "critical"`, `.abs > 3`,
        # `!event.within_limit`), never a bare-name predicate the way
        # `given`'s `Ports::Extraction.canonical` already handles. This
        # is a REAL gap, not equivalent to the already-built `where
        # field: value` sibling just above -- `where` is equality-only
        # on a literal hash ; `condition` needs the canonical
        # expression evaluator's comparison/`!`/arithmetic grammar
        # (confirmed present for `given`/`expects`, per this session's
        # bin-buddy findings) wired to a NAMED block param instead of
        # bare-attribute resolution, which nothing here does yet.
        # Accepted so the file boots, block body NOT evaluated --
        # meaning any policy using `condition` currently fires
        # UNCONDITIONALLY on its `on:` event, not gated as its own
        # bluebook declares. Deliberately not rushed : precisely
        # diagnosed and documented rather than silently treated as
        # equivalent to a real implementation. TODO upstream via
        # bin/evolve (migration plan task 7): give `condition` real
        # semantics, reusing `given`'s canonical-extraction machinery
        # once it can resolve a block param's dotted event-payload
        # access, not just bare names.
        def condition(&) = nil

        # Vendored no-op stub, not (yet) upstream hecksagain (migration
        # plan task 8): `cross_domain true` -- 3 occurrences, 2 files
        # (universal_domain_pattern.bluebook, causal_combinatorics.bluebook
        # -- both in hecks_nursury's meta-domain cluster, about Bluebook-
        # the-language itself rather than business modeling, per this
        # migration's own inventory). A bare boolean flag, distinct from
        # the already-real `across "DomainName"` (`@target_domain`) --
        # this names NO target domain at all, just marks the policy as
        # crossing a boundary conceptually. Accepted so the file boots ;
        # not wired to `@target_domain` or anything else, since there is
        # no domain name here to wire it TO. TODO upstream via bin/evolve
        # (migration plan task 7).
        def cross_domain(*) = nil

        def build
          IR::Policy.new(
            name:            @name,
            on_event:        @on_event,
            trigger_command: @trigger_command,
            target_domain:   @target_domain,
            with_literals:   @with_literals || {},
            wheres:          @wheres || {},
            for_each:        @for_each
          )
        end

        def self.build(name, &block)
          builder = new(name)
          builder.instance_eval(&block) if block
          builder.build
        end
      end
    end
  end
end
