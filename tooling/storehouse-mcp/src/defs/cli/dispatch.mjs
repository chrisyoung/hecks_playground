// dispatch.mjs — storehouse__dispatch
//             -> hecksagain-cli dispatch <aggregates_dir> <Verb> k=v ...
//
// REWRITTEN for hecksagain (migration plan Part 5): the old version
// scraped cascade/timeline log lines out of the Rust binary's
// human-readable stdout (parseEvents / dispatch_digest.mjs) because that
// was the only structured signal available. hecksagain-cli instead prints
// clean JSON directly ({ok, state, events} or {ok:false, error,
// error_class}) -- no scraping needed. This is a real simplification, not
// just parity: less fragile than parsing text meant for a terminal.
//
// The universal door : dispatch any bluebook command or query against any
// aggregates root. The runtime hydrates the .heki stores under
// aggregates_dir, applies the command, persists, and returns the
// resulting state as JSON.

import { z } from "zod";
import { spawn } from "node:child_process";

const HECKSAGAIN_CLI =
  process.env.HECKSAGAIN_CLI ||
  new URL("../../../../../hecksagain_runtime/bin/hecksagain-cli", import.meta.url).pathname;

function encodeAttrs(args) {
  const out = [];
  for (const [k, v] of Object.entries(args || {})) {
    if (v === undefined || v === null) continue;
    const encoded = typeof v === "object" ? JSON.stringify(v) : String(v);
    out.push(`${k}=${encoded}`);
  }
  return out;
}

function runDispatch(aggregatesDir, command, attrArgs) {
  return new Promise((resolve, reject) => {
    const argv = ["dispatch", aggregatesDir, command, ...attrArgs];
    const startedAt = Date.now();
    const child = spawn(HECKSAGAIN_CLI, argv, { stdio: ["ignore", "pipe", "pipe"] });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (d) => (stdout += d.toString()));
    child.stderr.on("data", (d) => (stderr += d.toString()));
    child.on("error", reject);
    child.on("close", (code) => {
      const duration_ms = Date.now() - startedAt;
      let parsed;
      try {
        parsed = JSON.parse(stdout.trim());
      } catch {
        parsed = { ok: false, error: "unparseable hecksagain-cli output", raw: stdout, stderr };
      }
      resolve({ ...parsed, command, aggregates_dir: aggregatesDir, duration_ms, exit_code: code });
    });
  });
}

function render(result) {
  const headline = result.ok ? `✓ ${result.command}` : `✗ ${result.command} — ${result.error || "failed"}`;
  const lines = [headline, `  ${result.duration_ms}ms`];
  // THE EXECUTION PORT'S REPLY, FIRST — present only when the dispatched
  // command's aggregate binds an `executed_by` adapter (Tools::ShellTool /
  // FileTool / SearchTool). It leads because for a tool call the output IS
  // the answer; the record and its events are the audit trail around it.
  //
  // Without this the door executes correctly and still LOOKS dead: `state`
  // echoes back only the attributes that were dispatched in, and the events
  // list prints names without payloads, so a shell command that genuinely
  // ran would surface no stdout anywhere. That was the shape of the
  // original bug and it must not be reproducible one layer up.
  if (result.reply) {
    const { tool, output, exit_code: exitCode, ok, cascade_error: cascadeError } = result.reply;
    lines.push("", `Output (${tool}, exit ${exitCode}${ok ? "" : " — FAILED"}):`, output ?? "");
    if (cascadeError) lines.push(`  [outcome not recorded: ${cascadeError}]`);
  }
  if (result.events?.length) {
    lines.push("", "Events:");
    result.events.forEach((e) => lines.push(`  ${e.name} (${e.aggregate}#${e.id})`));
  }
  if (result.state) {
    lines.push("", "State:", JSON.stringify(result.state, null, 2));
  }
  if (!result.ok) {
    lines.push("", `${result.error_class || "Error"}: ${result.error}`);
  }
  return lines.join("\n");
}

export default {
  name: "storehouse__dispatch",
  title: "Dispatch a Bluebook Command or Query",
  description:
    "The universal door. Dispatch any bluebook command or query against any aggregates root. Runs hecksagain-cli dispatch <aggregates_dir> <command> k=v ... command is the FQN — PascalCase for commands (e.g. 'Sandbox::Session.RecordNote') or snake_case for queries. args is an object of attribute key/values. Returns the runtime's state JSON in structuredContent.state. Use storehouse__catalog or storehouse__describe_aggregate first to discover what's callable.",
  inputSchema: {
    aggregates_dir: z
      .string()
      .min(1)
      .describe("Absolute path to the aggregates root, or a single domain's own directory (containing a bluebook/ subdir)."),
    command: z
      .string()
      .min(1)
      .describe("Fully-qualified verb — Domain::Aggregate.Command for commands, Domain::Aggregate.snake_case for queries."),
    args: z
      .record(z.any())
      .optional()
      .describe("Key/value pairs for the command's attributes."),
    summary: z
      .string()
      .min(1, "summary is required")
      .describe("One-line, terse summary of what this dispatch DOES. Required."),
  },
  async run(input) {
    const trimmedSummary = (input.summary || "").trim();
    if (!trimmedSummary) {
      return {
        content: [{ type: "text", text: "summary is required: provide a one-line description of what this dispatch does" }],
        isError: true,
      };
    }
    const attrArgs = encodeAttrs(input.args || {});
    const result = await runDispatch(input.aggregates_dir, input.command, attrArgs);
    return {
      content: [{ type: "text", text: render(result) }],
      isError: result.ok === false,
    };
  },
};
