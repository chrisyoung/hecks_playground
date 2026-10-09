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
        stage_bluebooks(source, dir)
        if include_hecksagons
          sibling = File.join(File.dirname(File.dirname(source)), "hecksagons")
          Dir.glob(File.join(sibling, "*.hecksagon")).each { |f| FileUtils.cp(f, File.join(dir, File.basename(f))) }
        end
        copy_port_wiring(source, dir)
        dir
      end

      # The source bluebook plus any same-domain sibling it reaches through
      # `reference_to` / `belongs_to` / `has_many` / `has_one` (conductor/claim.bluebook
      # points at worker.bluebook, which a lone copy cannot resolve). Siblings of another
      # domain, and same-domain ones nothing points at, stay out so a test still boots
      # only what it exercises. Files are numbered in dependency order because a bluebook
      # validates itself as it loads.
      def stage_bluebooks(source, dir)
        domain = HecksagainRuntime.domain_name_of(source)
        pool   = Dir.glob(File.join(File.dirname(source), "*.bluebook")).reject { |f| f == source }
                    .select { |f| HecksagainRuntime.domain_name_of(f) == domain }
        wanted = [source]
        queue  = [source]
        until queue.empty?
          needs = File.read(queue.shift).scan(/^\s*(?:reference_to|belongs_to|has_many|has_one)\s+(\w+)/).flatten
          (pool - wanted).each do |f|
            next unless (File.read(f).scan(/^\s*aggregate\s+"(\w+)"/).flatten & needs).any?

            wanted << f
            queue << f
          end
        end
        HecksagainRuntime.ordered_files(wanted).each_with_index do |f, i|
          FileUtils.cp(f, File.join(dir, wanted.size == 1 ? File.basename(f) : format("%03d_%s", i, File.basename(f))))
        end
      end

      # A bluebook whose queries are answered by a port boots only with that port bound, so
      # `<stem>.ports.hecksagon` and the adapters beside it (`*.adapter` with its `.rb`) travel
      # with the bluebook into the isolated dir, along with the `context_map.hecksagon` an
      # `attaches` needs. A hecksagon makes every aggregate's persistence an explicit decision,
      # so the copy is given the memory binds a test runs on (the bluebook's real stores stay in
      # its own hecksagon, which is not copied).
      def copy_port_wiring(source, dir)
        beside = File.dirname(source)
        ports  = File.join(beside, "#{File.basename(source, '.bluebook')}.ports.hecksagon")
        return unless File.file?(ports)

        [ports, File.join(beside, "context_map.hecksagon"), *Dir.glob(File.join(beside, "*.adapter")),
         *Dir.glob(File.join(beside, "*.rb"))].select { |f| File.file?(f) }.each do |f|
          FileUtils.cp(f, File.join(dir, File.basename(f)))
        end
        text   = File.read(source)
        domain = text[/Hecks\.bluebook\s+"(\w+)"/, 1]
        binds  = text.scan(/^\s*aggregate\s+"(\w+)"/).flatten.map { |a| "  #{domain}::#{a}.persisted_by(\"Memory\")\n" }
        File.write(File.join(dir, "memory_binds.hecksagon"), "Hecks.hecksagon \"#{domain}\" do\n#{binds.join}end\n")
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
