// validate.mjs — storehouse__validate → hecksagain-cli validate <aggregates_dir>
//
// REWRITTEN for hecksagain (migration plan Part 5): the old version
// shelled the retired Rust `storehouse validate <bluebook-path>` against
// a SINGLE FILE. hecksagain-cli's `validate` subcommand only ever takes a
// corpus ROOT directory (HecksagainRuntime.validate boots the whole
// staged domain via HecksPlayground.boot -- there is no single-file validate path;
// a bare-symbol identified_by or a cross-file reference can only be
// judged once the domain assembles). This is also, per Part 1's own
// validator finding, a real capability upgrade at the same time: a clean
// `HecksPlayground.boot` already IS validity (the DSL builders raise inline,
// Registry#verify! checks wiring at boot) -- there is no separate
// validation pass to reimplement, just "did boot raise."
//
// Output shape: hecksagain-cli always exits with `ok:true` (the CLI
// invocation itself succeeded) and carries the real verdict in `valid`
// (`{"ok":true,"valid":true}` or `{"ok":true,"valid":false,"error":...,
// "error_class":...}` on a genuine domain defect) -- NOT the old Rust
// binary's "VALID — Foo (N aggregates)" stdout line.

import { z } from "zod";
import { runCli, toMcpResponse } from "../../cli_dispatch.mjs";

export default {
  name: "storehouse__validate",
  title: "Validate a Bluebook Corpus",
  description:
    "Boot the domain at an aggregates root and report whether it boots clean. Runs hecksagain-cli validate <aggregates_dir> -- a clean HecksPlayground.boot IS validity (hecksagain's DSL builders raise inline, Registry#verify! checks wiring at boot time). Returns {ok:true,valid:true} on success or {ok:true,valid:false,error,error_class} on a real domain defect. Use this before macrophage_check or behaviors.",
  inputSchema: {
    aggregates_dir: z
      .string()
      .min(1)
      .describe(
        "Absolute path to the aggregates root, or a single domain's own directory (containing a bluebook/ subdir). NOT a single .bluebook file -- hecksagain validates a whole corpus, not one file in isolation.",
      ),
  },
  async run(args) {
    const result = await runCli("validate", [args.aggregates_dir]);
    return toMcpResponse(result);
  },
};
