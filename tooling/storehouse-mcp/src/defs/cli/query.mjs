// query.mjs — storehouse__query
//          -> hecksagain-cli query <root-or-bluebook> <verb> k=v ...
//
// REWRITTEN for hecksagain (migration plan Part 5): same {ok, rows} /
// {ok:false, error} clean-JSON shape as dispatch.mjs, no stdout scraping.
//
// Read-only counterpart to storehouse__dispatch. This tool guards the
// query SHAPE in-process: the verb-tail (after the last dot) must be
// snake_case; a PascalCase tail reads as a command and is rejected here
// with a pointer back at storehouse__dispatch.

import { z } from "zod";
import { spawn } from "node:child_process";

const HECKSAGAIN_CLI =
  process.env.HECKSAGAIN_CLI ||
  new URL("../../../../../hecksagain_runtime/bin/hecksagain-cli", import.meta.url).pathname;

function encodeAttrs(obj) {
  const out = [];
  for (const [k, v] of Object.entries(obj || {})) {
    if (v === undefined || v === null) continue;
    const encoded = typeof v === "object" ? JSON.stringify(v) : String(v);
    out.push(`${k}=${encoded}`);
  }
  return out;
}

function runQuery(root, verb, attrArgs) {
  return new Promise((resolve, reject) => {
    const argv = ["query", root, verb, ...attrArgs];
    const child = spawn(HECKSAGAIN_CLI, argv, { stdio: ["ignore", "pipe", "pipe"] });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (d) => (stdout += d.toString()));
    child.stderr.on("data", (d) => (stderr += d.toString()));
    child.on("error", reject);
    child.on("close", (code) => {
      let parsed;
      try {
        parsed = JSON.parse(stdout.trim());
      } catch {
        parsed = { ok: false, error: "unparseable hecksagain-cli output", raw: stdout, stderr };
      }
      resolve({ ...parsed, root, verb, exit_code: code });
    });
  });
}

export default {
  name: "storehouse__query",
  title: "Dispatch a Query (read-only)",
  description:
    "Dispatch a query against an aggregates root. Read-only. Runs hecksagain-cli query <root> <Domain::Aggregate.snake_case> k=v ... The verb-tail must be snake_case (commands use storehouse__dispatch). Use storehouse__catalog or storehouse__describe_aggregate to discover query names.",
  inputSchema: {
    aggregates_dir: z
      .string()
      .min(1)
      .describe("Absolute path to the aggregates root, or a single domain's own directory."),
    verb: z
      .string()
      .min(1)
      .describe("Fully-qualified query verb — Domain::Aggregate.snake_case (lowercase first letter after the dot)."),
    args: z
      .record(z.any())
      .optional()
      .describe("Key/value pairs for the query's input parameters."),
    summary: z
      .string()
      .min(1)
      .describe("One-line, terse summary of what this query DOES. Required."),
  },
  async run(input) {
    const trimmedSummary = (input.summary || "").trim();
    if (!trimmedSummary) {
      return {
        content: [{ type: "text", text: "summary is required: provide a one-line description of what this query does" }],
        isError: true,
      };
    }
    const verbTail = (input.verb || "").split(".").pop() || "";
    if (/^[A-Z]/.test(verbTail)) {
      return {
        content: [{ type: "text", text: `verb "${input.verb}" looks like a command (the segment after the last '.' is PascalCase). Queries are Domain::Aggregate.snake_case ; use storehouse__dispatch for commands.` }],
        isError: true,
      };
    }
    const attrArgs = encodeAttrs(input.args || {});
    const result = await runQuery(input.aggregates_dir, input.verb, attrArgs);
    const outputText = result.ok
      ? JSON.stringify(result.rows ?? result, null, 2)
      : `${result.error_class || "Error"}: ${result.error}`;
    return {
      content: [{ type: "text", text: outputText }],
      structuredContent: { ...result, summary: trimmedSummary },
      isError: !result.ok,
    };
  },
};
