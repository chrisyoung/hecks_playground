# HecksagainRuntime::BehaviorsRunner::Dispatching
#
# WHAT a test boots and HOW it calls the runtime -- the two decisions
# `Expectations` needs made for it before it can assert anything. Split
# out of expectations.rb by concern (and to keep both under the 200-LOC
# rule): that file is now purely "run one test and check what came back".
#
#   dir  = Dispatching.isolated_dir_for(source)
#   verb = Dispatching.qualify("Add", "Task", "Plan", bluebook)
#   Dispatching.dispatch_command(runtime, verb, file_path: "x")
#
# Usage is always through the module functions; nothing here holds state.
module HecksagainRuntime
  module BehaviorsRunner
    module Dispatching
      module_function

      # `on_aggregate` is the right guess for MOST setups (the corpus's own
      # convention is a chain of same-aggregate lifecycle steps before the
      # command under test). It is the WRONG guess for a genuine
      # cross-aggregate cascade test -- `tests "Add", on: "Task"` seeding a
      # `setup "Capture"` that actually creates the owning Story, found live
      # in plan.behaviors's own "Task.Add cascades through BumpOnTaskAdded"
      # test. So: try `on_aggregate` first (cheap, right almost always) ;
      # if that aggregate doesn't declare the command, search every
      # aggregate the domain actually has for one that does, and use THAT
      # instead of guessing wrong silently. Falls back to the original
      # guess (so the resulting UnknownVerb names the aggregate the author
      # meant) only if truly no aggregate declares it.
      def qualify(command, on_aggregate, domain_name, bluebook)
        return command.to_s if command.to_s.include?(".")

        resolved = on_aggregate
        target   = bluebook&.aggregate(on_aggregate)
        if bluebook && !(target && target.command(command))
          owner = bluebook.aggregates.find { |a| a.command(command) }
          resolved = owner.name if owner
        end
        "#{domain_name}::#{resolved}.#{command}"
      end

      # `include_hecksagons:` (slice 1.3) -- see Expectations#run_one's own
      # comment on the two kinds that pass true. The sibling folder sits
      # one level up from `source`'s own directory (`.../tools/bluebook/
      # tools.bluebook` -> `.../tools/hecksagons/`), the exact convention
      # tools.behaviors's own header names.
      def isolated_dir_for(source, include_hecksagons: false)
        dir = Dir.mktmpdir("behaviors-")
        FileUtils.cp(source, File.join(dir, File.basename(source)))
        if include_hecksagons
          sibling = File.join(File.dirname(File.dirname(source)), "hecksagons")
          Dir.glob(File.join(sibling, "*.hecksagon")).each { |f| FileUtils.cp(f, File.join(dir, File.basename(f))) }
        end
        dir
      end

      # A behaviors test writes a dispatch with receiver identity and
      # command facts side by side (`label: "g", id: "wn", to: {...}`), but
      # since hecks #335 the dispatcher's own `to:` keyword is the ROUTING
      # envelope -- so forwarding those kwargs loose collides the moment a
      # domain declares a command fact named `to`. This corpus does:
      # Tools::MailTool.CreateDraft takes a `to`, and every test of it
      # failed with "to: does not recognize value" once the tests started
      # running at all. `ReactionInvocation.build` is #335's own seam for
      # turning mixed facts into the strict envelope (identities lifted
      # into `to:`, declared facts into `with:`), so a behaviors dispatch
      # now goes through the exact same separation a policy's projection
      # does. Ported from the published gem's own
      # Hecks::Behaviors::Expectations, which solved this first -- not
      # reinvented.
      #
      # A verb naming a PORT OPERATION keeps the loose passthrough: its
      # input already spells the port form's `to:`/`with:`, which the
      # dispatcher's port branch reads directly, while
      # `ReactionInvocation.build` expects a command's own declared
      # attributes at the top level instead.
      def dispatch_command(runtime, verb, args)
        return runtime.dispatch(verb, **args) if port_operation?(runtime, verb)

        invocation = begin
          Hecks::Runtime::ReactionInvocation.build(registry: runtime.registry, verb: verb,
                                                   projected: args, explicit: true)
        rescue Hecks::Runtime::UnknownVerb
          nil
        end
        return runtime.dispatch(verb, **args) unless invocation

        if invocation.key?(:to)
          runtime.dispatch(verb, to: invocation[:to], with: invocation[:with])
        else
          runtime.dispatch(verb, with: invocation[:with])
        end
      end

      # The same "Head.Rest" shape `Dispatcher#dispatch` and
      # `ReactionInvocation#resolve_target` both already check -- a bare
      # domain/aggregate lookup plus a port-name lookup, no command
      # resolution needed since all this asks is whether one exists.
      def port_operation?(runtime, verb)
        domain, aggregate_name, command_path = Hecks::Naming.split_verb(verb)
        return false unless command_path

        aggregate = runtime.registry.bluebook(domain)&.aggregate(aggregate_name)
        return false unless aggregate

        head, rest = command_path.split(".", 2)
        rest && !!aggregate.port(head)
      rescue StandardError
        false
      end
    end
  end
end
