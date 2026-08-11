// conceive_behaviors.mjs — storehouse__conceive_behaviors
//                       → hecksagain-cli conceive-behaviors <path>
//
// REWRITTEN for hecksagain (migration plan Part 5 / Part 1 item 5,
// "correction to an earlier claim" pass): same documented call as
// behaviors.mjs's sibling -- hecksagain has no `.behaviors` companion
// generator (the retired Rust binary's behaviors_conceiver/ has no Ruby
// port), and hecksagain's own testing convention (RSpec + JSON corpus
// fixtures) isn't a superset this could translate into instead. See
// behaviors.mjs's header for the full reasoning ; not repeated here.
//
// hecksagain-cli's `conceive-behaviors` subcommand returns a clear,
// honest {ok:false, supported:false, error} rather than crashing or
// silently no-opping.

import { z } from "zod";
import { runCli, toMcpResponse } from "../../cli_dispatch.mjs";

export default {
  name: "storehouse__conceive_behaviors",
  title: "Generate a Behaviors Companion (not yet supported under hecksagain)",
  description:
    "Generate a behaviors companion (sample-coverage tests) for a bluebook. NOT YET SUPPORTED under hecksagain (migration plan Part 1 item 5) -- hecksagain has no .behaviors generator ; its own testing convention is RSpec + JSON corpus fixtures instead. Runs hecksagain-cli conceive-behaviors <bluebook_path>, which returns {ok:false, supported:false, error} explaining the gap rather than crashing or silently no-opping.",
  inputSchema: {
    bluebook_path: z
      .string()
      .min(1)
      .describe("Absolute path to the source bluebook."),
  },
  async run(args) {
    const result = await runCli("conceive-behaviors", [args.bluebook_path]);
    return toMcpResponse(result);
  },
};
