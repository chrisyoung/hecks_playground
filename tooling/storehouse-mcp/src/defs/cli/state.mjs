// state.mjs — storehouse__state
//          → hecksagain-cli state <aggregates_dir> <Domain::Aggregate> <id>
//
// REWRITTEN for hecksagain (migration plan Part 5): the plan's own prose
// flagged that hecksagain has no single-record-by-id `bin/*` tool
// (`bin/stores` dumps every aggregate's current head records, not
// filtered to one) and recommended reusing its underlying approach --
// `registry.repository(...).find(id)` -- rather than a new subsystem.
// HecksagainRuntime.state does exactly that (a few lines, matching Part
// 5's own sizing call). This tool's own param shape (aggregates_dir +
// positional call order) already matched the new contract before this
// pass ; what changed is the aggregate_name description -- hecksagain's
// `state` subcommand splits the FQN on "::" (Domain::Aggregate), unlike
// the old Rust binary's bare aggregate name.
//
// Direct read of one aggregate instance's current runtime state by id.
// Boots the runtime against the aggregates root and emits
// { ok, aggregate, id, state } as JSON. A missing record returns
// ok:false with state:null (not an error — "no such record" is a normal
// answer to a state query).

import { z } from "zod";
import { runCli, toMcpResponse } from "../../cli_dispatch.mjs";

export default {
  name: "storehouse__state",
  title: "Read Aggregate State by ID",
  description:
    "Read the current state of one aggregate instance by its id. Boots the runtime against the aggregates root and returns { ok, aggregate, id, state } as JSON. ok=false with state=null when no record matches (not an error). Runs hecksagain-cli state <aggregates_dir> <Domain::Aggregate> <id>.",
  inputSchema: {
    aggregates_dir: z
      .string()
      .min(1)
      .describe(
        "Absolute path to the aggregates root, or a single domain's own directory (where the .heki stores live).",
      ),
    aggregate_name: z
      .string()
      .min(1)
      .describe(
        "Fully-qualified aggregate name, Domain::Aggregate (e.g. 'Tools::ShellTool'). Case-sensitive.",
      ),
    id: z
      .string()
      .min(1)
      .describe("The aggregate instance's id."),
    summary: z
      .string()
      .min(1)
      .describe(
        "One-line, terse summary of why you are reading this state — e.g., 'verify ShellTool record persisted after bash dispatch'. Recommended ≤80 characters. Required.",
      ),
  },
  async run(args) {
    const trimmedSummary = (args.summary || "").trim();
    if (!trimmedSummary) {
      return {
        content: [{ type: "text", text: "summary is required: provide a one-line description of why you are reading this state (e.g. 'confirm record exists after dispatch')" }],
        isError: true,
      };
    }
    const result = await runCli("state", [
      args.aggregates_dir,
      args.aggregate_name,
      args.id,
    ]);
    return toMcpResponse(result);
  },
};
