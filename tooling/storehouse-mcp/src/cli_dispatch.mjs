// cli_dispatch.mjs
//
// StorehouseCliRunner — sibling of dispatch.mjs.
//
// dispatch.mjs shells `storehouse <root> <Verb> k=v ...` (the bus
// dispatcher form, used by Tools.* tools).
//
// cli_dispatch.mjs shells `storehouse <subcommand> <bluebook-path> [args...]`
// (the developer CLI form, used by validate / list / macrophage / behaviors
// / conceive / dispatch_command).
//
// Returns a uniform shape :
//   { ok: bool, exit_code: int, stdout: string, stderr: string,
//     subcommand: string, args: [...] }
//
// Env vars :
//   STOREHOUSE_BIN  — path to the storehouse binary (default: "storehouse")
//
// Usage :
//   import { runCli } from "./cli_dispatch.mjs";
//   const r = await runCli("validate", ["/path/to/foo.bluebook"]);
//   // → { ok: true, exit_code: 0, stdout: "VALID — Foo (1 aggregates)\n", ... }

import { spawn } from "node:child_process";

// REWRITTEN for hecksagain (migration plan Part 5): HECKSAGAIN_CLI
// replaces STOREHOUSE_BIN as the pointed-at binary -- a Ruby script over
// hecksagain_runtime, not the retired Rust `storehouse` binary. The
// runCli/toMcpResponse/toMcpResponsePretty SHAPE stays because it's
// already generic (a subprocess spawner + response wrapper, not opinionated
// about the old binary's specific subcommands) -- only the target changes.
// Migration plan task 5 (Task 5 verification pass, 2026-08-11): this was
// `../../hecksagain_runtime/...` -- two levels up from
// tooling/storehouse-mcp/src/ lands at tooling/, not the migration root,
// so every tool routed through this shared helper (catalog/describe_
// aggregate/list_aggregates/state/validate/behaviors) spawned a
// nonexistent tooling/hecksagain_runtime/bin/hecksagain-cli and failed
// with ENOENT -- caught live by actually running the worktree's own
// `npm run smoke` end-to-end test, not by reading the code. dispatch.mjs/
// query.mjs compute their own path independently (5 levels, correctly)
// and were unaffected -- this file needed a THIRD `../` to reach the
// same migration root they do.
const HECKSAGAIN_CLI =
  process.env.HECKSAGAIN_CLI ||
  new URL("../../../hecksagain_runtime/bin/hecksagain-cli", import.meta.url).pathname;

export async function runCli(subcommand, args = [], opts = {}) {
  const fullArgs = [subcommand, ...args];
  return await new Promise((resolve, reject) => {
    const child = spawn(HECKSAGAIN_CLI, fullArgs, {
      env: { ...process.env, ...(opts.env || {}) },
      stdio: opts.stdin ? ["pipe", "pipe", "pipe"] : ["ignore", "pipe", "pipe"],
    });

    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (d) => (stdout += d.toString()));
    child.stderr.on("data", (d) => (stderr += d.toString()));

    if (opts.stdin) {
      child.stdin.write(opts.stdin);
      child.stdin.end();
    }

    child.on("error", reject);
    child.on("close", (code) => {
      resolve({
        ok: code === 0,
        exit_code: code,
        stdout,
        stderr,
        subcommand,
        args: fullArgs,
      });
    });
  });
}

// Helper : wrap a runCli result into the MCP tool response envelope. Used
// by every storehouse__<cli-tool> def so they share one format.
export function toMcpResponse(result) {
  const summary = result.stdout || result.stderr || `exit=${result.exit_code}`;
  return {
    content: [
      {
        type: "text",
        text: summary,
      },
    ],
    structuredContent: result,
    isError: result.ok === false,
  };
}

// Like toMcpResponse but attempts to parse the stdout as JSON and
// re-emit it with 2-space indentation. Used for discovery tools
// (catalog, describe_aggregate) that return structured data — makes
// the response readable in the MCP inspector and downstream renderers.
// Falls back to raw stdout if parsing fails (e.g. error messages).
export function toMcpResponsePretty(result) {
  let text = result.stdout || result.stderr || `exit=${result.exit_code}`;
  if (result.stdout) {
    try {
      const parsed = JSON.parse(result.stdout.trim());
      text = JSON.stringify(parsed, null, 2);
    } catch {
      // not valid JSON (e.g. error output) — fall through to raw text
    }
  }
  return {
    content: [
      {
        type: "text",
        text,
      },
    ],
    structuredContent: result,
    isError: result.ok === false,
  };
}
