// macrophage_check.mjs — storehouse__macrophage_check
//                     -> hecksagain-cli macrophage <aggregates_dir> (stdin: tool-input payload)
//
// REWRITTEN for hecksagain (migration plan Part 5): the old tool had no
// explicit root/aggregates_dir param — hecksagain-cli's macrophage
// subcommand needs one (HecksagainRuntime::MacrophageBridge.call(root,
// payload) dispatches Macrophage::* commands, which needs to know WHERE
// the Macrophage domain lives). Added as a required param rather than
// hardcoding a default.
//
// The macrophage is a PostToolUse-style enforcer that reads a tool-input
// payload from stdin and complains if the touched file isn't a bluebook
// (or is an unexempted imperative edit). For an on-demand CLI check we
// synthesize the stdin payload with the bluebook_path so callers can
// spot-check the macrophage's verdict without running it as a hook.

import { z } from "zod";
import { runCli, toMcpResponse } from "../../cli_dispatch.mjs";

export default {
  name: "storehouse__macrophage_check",
  title: "Run Macrophage on a Bluebook",
  description:
    "Run the macrophage discipline check against a file path. Runs hecksagain-cli macrophage <aggregates_dir> with a synthesized tool-input payload on stdin. Returns the macrophage's state JSON ; complaints surface under state.last_complaint.",
  inputSchema: {
    aggregates_dir: z
      .string()
      .min(1)
      .describe("Absolute path to the aggregates root the Macrophage domain lives under (where governance .heki stores persist)."),
    bluebook_path: z
      .string()
      .min(1)
      .describe("Absolute path to the file the macrophage should evaluate (typically a .bluebook)."),
  },
  async run(args) {
    const payload = JSON.stringify({
      tool_input: { file_path: args.bluebook_path },
    });
    const result = await runCli("macrophage", [args.aggregates_dir], { stdin: payload });
    return toMcpResponse(result);
  },
};
