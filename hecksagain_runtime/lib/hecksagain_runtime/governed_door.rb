# governed_door.rb — LookupDoor's hand-written Ruby escape hatch.
#
# hecksagain's query interpreter has no mechanism for a query that
# introspects the bluebook's own command IR instead of stored records
# (confirmed by reading QueryInterpreter/ReadModelInterpreter end to end --
# see Part 5 of the migration plan). The old Rust runtime special-cased this
# exact query the same way; this is that same special-case, in Ruby, outside
# the ordinary dispatch path.
#
# Scans every loaded bluebook's every command for `redirects_native` (see
# hecksagain_runtime.rb's IR::Command patch) containing the queried tool
# name, and returns the fully-qualified door command plus door_args derived
# from that command's own attributes -- mirroring what the Rust binary did
# for `storehouse query <root> Macrophage::GovernedDoor.lookup_door tool=X`.
#
# Fails CLEANLY (returns {door: nil}) rather than raising, on purpose --
# governed-door-hook's own documented policy is fail-OPEN on a lookup error
# ("a wedged governance machine must never brick the session"). A raise here
# would still fail open at the CALLER (the hook treats a non-zero exit /
# unparseable reply as "no door found"), but returning cleanly is the more
# honest contract for a library method.

require "hecks"

module HecksagainRuntime
  module GovernedDoor
    def self.lookup_door(root, tool)
      runtime = Hecks.boot(HecksagainRuntime.stage_flat_corpus(root), install_facade: false)
      runtime.registry.bluebooks.each_value do |bluebook|
        bluebook.aggregates.each do |agg|
          agg.commands.each do |cmd|
            natives = cmd.redirects_native || []
            next unless natives.include?(tool)

            return {
              ok: true,
              door: true,
              door_equivalent: "#{bluebook.name}::#{agg.name}.#{cmd.hecks_playground_name}",
              door_args: cmd.attributes.map(&:name),
            }
          end
        end
      end
      { ok: true, door: false }
    rescue StandardError => e
      # Fail open, per governed-door-hook's documented policy -- but SAY so,
      # never silently pretend nothing was governed.
      { ok: false, door: false, error: e.message, error_class: e.class.name }
    end
  end
end
