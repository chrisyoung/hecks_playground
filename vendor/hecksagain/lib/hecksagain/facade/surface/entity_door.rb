require_relative "../handle"
require_relative "../../naming"

module Hecksagain
  module Facade
    module Surface
      # A nested entity's own door — never installed at boot the way an
      # aggregate's is (`Surface.install`'s own `Namespace.install(Object,
      # ...)`), only ever nested under the AGGREGATE it belongs to:
      # `Banking::Account::LedgerEntry.reverse!(...)`, never a bare
      # `Banking::LedgerEntry`. Built by `AggregateDoor#aggregate_module`,
      # one call per entity the aggregate declares — a nested entity (an
      # entity's own `entities`) gets the identical treatment one level
      # further in : an entity TWO deep, with a command of its own, would
      # resolve `Domain::Aggregate::Outer::Inner.command!(...)`, the exact
      # shape `Domain::Aggregate::Outer.command!(...)` already resolves
      # one hop up (no real corpus construct is two entities deep with a
      # command at the bottom yet — `EntityReference::Manifest::Crate`'s
      # own `Seal`, docs/reference/entity.md, nests a plain attribute-bag
      # entity NAMED `Seal` beside a COMMAND also named `Seal` on `Crate`
      # itself, two different namespaces sharing one word, not a
      # two-deep chain — but the recursion here handles one exactly the
      # same way regardless, the moment a bluebook declares it).
      #
      # EVERY entity command is a bang method here, none split out to an
      # instance-level Handle the way AggregateDoor splits creating vs.
      # non-creating verbs — an entity is NEVER created through its own
      # command (docs/guides/entities.md's own rule), so there is no
      # "non-creating, act on a found record" case to split off; every
      # command here acts on an element that already exists, addressed by
      # the full identity chain the caller passes as keyword args.
      #
      # `dispatcher.dispatch_entity`, never `dispatch`/`reenter` — the
      # ONLY door that reaches one (`Runtime::EntityDispatchRefused`'s own
      # header, errors.rb, explains why a verb string never does).
      module EntityDoor
        def entity_module(dispatcher, domain, root_fqn, root_ir, entity, dotted_prefix)
          dotted = dotted_prefix.empty? ? entity.hecks_name : "#{dotted_prefix}.#{entity.hecks_name}"
          door   = Module.new

          # THE RECORD IN HAND IS THE ROOT AGGREGATE'S, always —
          # `EntityInterpreter#call` answers `[ctx.instance, ctx.result]`
          # where `ctx.instance` is the PARENT AGGREGATE record no matter
          # how many entities deep the dotted chain reaches (this file's
          # own header comment on `Context`, entity_interpreter.rb) — so
          # a command wraps the result in a Handle keyed to `root_ir`,
          # the same aggregate `AggregateDoor`'s own creating verbs wrap.
          entity.commands.each do |command|
            door.define_singleton_method("#{Naming.snake(command.hecks_name)}!") do |**args|
              Handle.new(dispatcher: dispatcher, domain: domain, ir: root_ir,
                         instance: dispatcher.dispatch_entity("#{root_fqn}.#{dotted}.#{command.hecks_name}", **args).instance)
            end
          end

          entity.entities.each do |nested|
            door.const_set(nested.hecks_name, entity_module(dispatcher, domain, root_fqn, root_ir, nested, dotted))
          end

          door.define_singleton_method(:fqn)      { "#{root_fqn}.#{dotted}" }
          door.define_singleton_method(:commands) { entity.commands.map { |c| "#{Naming.snake(c.hecks_name)}!" }.sort }

          door
        end
      end
    end
  end
end
