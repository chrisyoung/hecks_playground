// catalog.mjs — storehouse__catalog → hecksagain-cli catalog <aggregates_dir>
//
// REWRITTEN for hecksagain (migration plan Part 5): the old version
// shelled the retired Rust `storehouse dump <bluebook-path>` against a
// SINGLE FILE. hecksagain-cli's `catalog` subcommand only ever takes a
// corpus ROOT directory (HecksagainRuntime.catalog boots the whole
// staged domain and returns `runtime.registry.bluebooks.transform_values
// (&:to_h)` -- every loaded bluebook's own IR, keyed by domain name).
// This is hecksagain's native Bluebook#to_h shape (ir_version, name,
// version, vision, classification, category, aggregates, read_models,
// policies, process_managers, canonical_form) -- a documented BREAKING
// shape change from the old dump.rs shape (no read_models, differently-
// shaped category), taken deliberately per Part 5's own call ("no
// backward compatibility until first user, break APIs freely"), not
// preserved for compatibility.
//
// Full canonical-IR JSON for a whole aggregates root : every domain's
// every aggregate, command, query, attribute, value object, policy. This
// is the LLM's index when constructing dispatch verbs through
// storehouse__dispatch.
//
// For one-aggregate focus, use storehouse__describe_aggregate.

import { z } from "zod";
import { runCli, toMcpResponsePretty } from "../../cli_dispatch.mjs";

export default {
  name: "storehouse__catalog",
  title: "Full Bluebook Catalog (canonical IR)",
  description:
    "Emit hecksagain's own IR JSON for every bluebook loaded from an aggregates root -- every aggregate, command, query, attribute, value object, policy, keyed by domain name. This is the LLM's index for discovering what storehouse__dispatch can call. Runs hecksagain-cli catalog <aggregates_dir>. NOTE (migration plan Part 5): this is hecksagain's native Bluebook#to_h shape (ir_version, classification, read_models, canonical_form), a documented breaking change from the old dump.rs shape (category, no read_models) — taken deliberately, not preserved for compatibility.",
  inputSchema: {
    aggregates_dir: z
      .string()
      .min(1)
      .describe(
        "Absolute path to the aggregates root, or a single domain's own directory (containing a bluebook/ subdir). NOT a single .bluebook file.",
      ),
  },
  async run(args) {
    const result = await runCli("catalog", [args.aggregates_dir]);
    return toMcpResponsePretty(result);
  },
};
