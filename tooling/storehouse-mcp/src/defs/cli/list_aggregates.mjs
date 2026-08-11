// list_aggregates.mjs — storehouse__list_aggregates
//                     → hecksagain-cli list <aggregates_dir>
//
// REWRITTEN for hecksagain (migration plan Part 5): the old version
// shelled the retired Rust `storehouse parse <bluebook-path>` against a
// SINGLE FILE. hecksagain-cli's `list` subcommand only ever takes a
// corpus ROOT directory (HecksagainRuntime.list_aggregates boots the
// whole staged domain and returns every loaded bluebook's aggregate
// names flattened into one array -- {ok:true, aggregates:[...]}).
// Bare names, not FQNs -- use storehouse__describe_aggregate with the
// Domain::Aggregate form once you know which domain a name belongs to.

import { z } from "zod";
import { runCli, toMcpResponse } from "../../cli_dispatch.mjs";

export default {
  name: "storehouse__list_aggregates",
  title: "List Aggregates in an Aggregates Root",
  description:
    "List the aggregate names defined across every bluebook loaded from an aggregates root. Runs hecksagain-cli list <aggregates_dir>. Use as a discovery probe before storehouse__dispatch.",
  inputSchema: {
    aggregates_dir: z
      .string()
      .min(1)
      .describe(
        "Absolute path to the aggregates root, or a single domain's own directory (containing a bluebook/ subdir). NOT a single .bluebook file.",
      ),
  },
  async run(args) {
    const result = await runCli("list", [args.aggregates_dir]);
    return toMcpResponse(result);
  },
};
