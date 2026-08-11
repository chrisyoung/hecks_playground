// behaviors.mjs — storehouse__behaviors → hecksagain-cli behaviors <path>
//
// REWRITTEN for hecksagain (migration plan Part 5 / Part 1 item 5,
// "correction to an earlier claim" pass): flagged from the start as
// ORTHOGONAL and unresolved -- hecksagain's own testing convention is
// plain RSpec + JSON corpus fixtures, a DIFFERENT format, not a superset
// of the `.behaviors` DSL the retired Rust `storehouse` binary ran.
// Checked directly this pass (not assumed): grepped both the vendored
// copy AND the untouched original ~/Projects/hecksagain for a
// BehaviorRuntime class or any bin/* script that executes a `.behaviors`
// file -- zero hits, either place. Writing a real Ruby interpreter for
// the `.behaviors` DSL is substantial, standalone scope (the same size
// class as the still-open bare-primitive-attribute decision from the
// earlier "largest real finding" pass), not a plumbing fix -- not
// attempted here.
//
// DOCUMENTED CALL: rather than leave this tool silently broken (an
// ENOENT from a nonexistent binary path, or a bare "unknown subcommand"
// with no explanation), hecksagain-cli now has a real `behaviors`
// subcommand that returns a clear, honest, non-crashing "not yet
// supported" answer naming exactly why. Not a silent no-op pretending
// success -- an explicit, structured refusal a caller can act on (fall
// back to RSpec, or pick this back up as its own scoped port).

import { z } from "zod";
import { runCli, toMcpResponse } from "../../cli_dispatch.mjs";

export default {
  name: "storehouse__behaviors",
  title: "Run a Behaviors File (not yet supported under hecksagain)",
  description:
    "Run all behaviors in a *.behaviors file. NOT YET SUPPORTED under hecksagain (migration plan Part 1 item 5) -- hecksagain has no .behaviors interpreter ; its own testing convention is RSpec + JSON corpus fixtures instead. Runs hecksagain-cli behaviors <path>, which returns {ok:false, supported:false, error} explaining the gap rather than crashing or silently no-opping. Kept as a real tool (not removed) so a caller gets an honest, actionable answer instead of a missing-tool error.",
  inputSchema: {
    behaviors_path: z
      .string()
      .min(1)
      .describe("Absolute path to a .behaviors file."),
  },
  async run(args) {
    const result = await runCli("behaviors", [args.behaviors_path]);
    return toMcpResponse(result);
  },
};
