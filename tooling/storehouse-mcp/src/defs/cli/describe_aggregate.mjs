// describe_aggregate.mjs — storehouse__describe_aggregate
//
// REWRITTEN for hecksagain (migration plan Part 5): the old version had
// no dedicated `describe` CLI subcommand, so it fetched the WHOLE domain
// dump and filtered client-side. hecksagain-cli exposes `describe`
// natively (HecksagainRuntime.describe_aggregate) — a real simplification,
// not just parity: one call instead of fetch-then-filter, and the "which
// names exist" miss message comes from the same place catalog's data does.
// Also matches the corpus-root contract every other rewritten tool now
// uses -- `bluebook_path` (a single file) is gone; `aggregates_dir` (a
// corpus root or single domain directory) plus a fully-qualified
// Domain::Aggregate name is what hecksagain-cli describe actually takes.
//
// Emits the IR for ONE aggregate inside a domain — commands, queries,
// attributes, references, value objects — before constructing a dispatch
// verb. For the full domain, use storehouse__catalog.

import { z } from "zod";
import { runCli, toMcpResponsePretty } from "../../cli_dispatch.mjs";

export default {
  name: "storehouse__describe_aggregate",
  title: "Describe One Aggregate",
  description:
    "Emit the IR for one aggregate loaded from an aggregates root. Shows commands, queries, attributes, references, value_objects, lifecycle states. Runs hecksagain-cli describe <aggregates_dir> <Domain::Aggregate>. Use as a focused discovery probe before storehouse__dispatch ; for the whole domain use storehouse__catalog.",
  inputSchema: {
    aggregates_dir: z
      .string()
      .min(1)
      .describe(
        "Absolute path to the aggregates root, or a single domain's own directory (containing a bluebook/ subdir). NOT a single .bluebook file.",
      ),
    aggregate_name: z
      .string()
      .min(1)
      .describe(
        "Fully-qualified aggregate name, Domain::Aggregate (case-sensitive). Get the available list from storehouse__list_aggregates.",
      ),
  },
  async run(args) {
    const result = await runCli("describe", [args.aggregates_dir, args.aggregate_name]);
    return toMcpResponsePretty(result);
  },
};
