// cli_smoke.mjs
//
// End-to-end smoke for the 10 storehouse__<cli> tools and the
// storehouse://events resource. Boots the server via stdio MCP
// transport, lists tools+resources, calls a few tools, subscribes to
// events, writes a line to the JSONL file, asserts the
// notifications/resources/updated frame fires.
//
// Run : `node test/cli_smoke.mjs` from the storehouse-mcp/ dir.
// Exits 0 on green, non-zero on assertion failure.

import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { writeFileSync, appendFileSync, unlinkSync, existsSync, mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const SERVER = path.join(__dirname, "..", "src", "server.mjs");
// Migration plan task 5: this pointed at the MAIN hecks repo, still on
// the old dialect and deliberately untouched until Task 6's cutover --
// hecksagain-cli correctly refuses it ("no such domain directory").
// This smoke test's whole job is verifying the PRE-cutover migration
// worktree's own rewritten hecks_conception, so it needs to point at
// the worktree, not the not-yet-cut-over main repo. Caught live by
// actually running `npm run smoke`, not by reading the code.
const HECKS_ROOT = "/Users/christopheryoung/Projects/hecks-hecksagain-migration";
// Migration plan Part 5 "correction" pass: hecksagain-cli's validate /
// catalog / describe / list / macrophage subcommands only ever accept a
// CORPUS ROOT directory, never a single .bluebook file (confirmed against
// every real invocation this whole migration has made) -- SMOKE_ROOT
// replaces the old SMOKE_BLUEBOOK single-file target for those probes.
// RestartPrompt (a small single-aggregate bluebook under this root) is
// still the known-good aggregate the discovery probes look for.
const SMOKE_ROOT = path.join(HECKS_ROOT, "hecks_conception");
const SMOKE_BEHAVIORS = path.join(
  HECKS_ROOT,
  "hecks_conception/aggregates/framework/restart_prompt/bluebook/restart_prompt.behaviors",
);
// Found live this pass (migration plan Part 5 "correction" pass, running
// the ACTUAL MCP tool surface for the first time rather than
// hecksagain-cli directly): Tools::ShellTool moved onto `event_sourced`
// sugar (Heki-backed, durable) in an earlier pass this session -- the
// smoke test's own prior comment block ("a tool invocation's CURRENT
// STATE is deliberately ephemeral... Memory-backed... does not outlive
// its process") describes a PRIOR corpus state, not the current one.
// Confirmed directly: `hecksagain-cli dispatch ... id=cli-smoke-bash`
// after any earlier run now fails `AlreadyExists` (Bash is a creates-only
// command per the corpus-wide `creates?` finding -- Part 3a's own
// dedicated, deliberately-unfixed section) because the id genuinely
// persists across process boundaries. A fixed literal id therefore made
// this whole smoke test non-idempotent -- green once, then red on every
// subsequent run, forever, until someone deleted the staged tmp corpus by
// hand. RUN_ID makes every dispatch id in this file unique per run so the
// suite stays repeatable, and the state assertions below now test the
// REAL current (durable) behaviour instead of a stale assumption.
const RUN_ID = Date.now();

let failed = 0;
function ok(label) { process.stderr.write(`ok   ${label}\n`); }
function fail(label, detail) {
  failed++;
  process.stderr.write(`FAIL ${label}: ${detail}\n`);
}
function eq(a, b, label) { a === b ? ok(label) : fail(label, `expected ${JSON.stringify(b)} got ${JSON.stringify(a)}`); }
function truthy(v, label) { v ? ok(label) : fail(label, `expected truthy got ${JSON.stringify(v)}`); }
function contains(haystack, needle, label) {
  haystack && haystack.includes(needle)
    ? ok(label)
    : fail(label, `did not find ${JSON.stringify(needle)} in ${JSON.stringify(haystack)?.slice(0, 200)}`);
}

async function main() {
  // Use a private events file so the test does not collide with the
  // real runtime stream.
  const tmpDir = mkdtempSync(path.join(tmpdir(), "storehouse-mcp-smoke-"));
  const eventsPath = path.join(tmpDir, "events.jsonl");
  writeFileSync(eventsPath, "");

  const transport = new StdioClientTransport({
    command: process.execPath,
    args: [SERVER],
    env: {
      ...process.env,
      HECKS_AGENT_EVENT_STREAM: eventsPath,
      STOREHOUSE_BIN: path.join(HECKS_ROOT, "rust/target/release/storehouse"),
    },
  });

  const client = new Client(
    { name: "storehouse-mcp-cli-smoke", version: "0.0.1" },
    { capabilities: {} },
  );
  await client.connect(transport);

  // -- tools/list contains the 10 expected tools
  const list = await client.listTools();
  const names = new Set(list.tools.map((t) => t.name));
  for (const expected of [
    "storehouse__dispatch",
    "storehouse__query",
    "storehouse__state",
    "storehouse__catalog",
    "storehouse__describe_aggregate",
    "storehouse__list_aggregates",
    "storehouse__validate",
    "storehouse__macrophage_check",
    "storehouse__behaviors",
    "storehouse__conceive_behaviors",
  ]) {
    truthy(names.has(expected), `tools/list has ${expected}`);
  }
  // The retired hand-registered tools must NOT come back.
  for (const gone of [
    "storehouse__bash",
    "storehouse__read",
    "storehouse__edit",
    "storehouse__update",
    "storehouse__grep",
    "storehouse__glob",
    "storehouse__web_fetch",
    "storehouse__web_search",
    "storehouse__record_result",
    "storehouse__dispatch_command",
    "storehouse__list",
  ]) {
    truthy(!names.has(gone), `retired tool ${gone} is gone`);
  }

  // -- resources/list returns storehouse://events
  const resources = await client.listResources();
  const uris = resources.resources.map((r) => r.uri);
  truthy(uris.includes("storehouse://events"), "resources/list has storehouse://events");

  // -- storehouse__validate on the known-good corpus root
  const validateRes = await client.callTool({
    name: "storehouse__validate",
    arguments: { aggregates_dir: SMOKE_ROOT },
  });
  eq(validateRes.isError, false, "validate isError=false");
  contains(validateRes.content[0].text, '"valid":true', "validate reports valid:true");

  // -- storehouse__catalog returns the full IR (JSON, pretty-printed)
  const catalogRes = await client.callTool({
    name: "storehouse__catalog",
    arguments: { aggregates_dir: SMOKE_ROOT },
  });
  eq(catalogRes.isError, false, "catalog isError=false");
  contains(catalogRes.content[0].text, '"aggregates"', "catalog stdout has aggregates key");
  // pretty-printed JSON has leading spaces (2-space indent)
  truthy(catalogRes.content[0].text.includes("  "), "catalog output is pretty-printed (has indentation)");

  // -- storehouse__describe_aggregate emits one aggregate's IR
  const describeRes = await client.callTool({
    name: "storehouse__describe_aggregate",
    arguments: { aggregates_dir: SMOKE_ROOT, aggregate_name: "RestartPrompt::RestartPrompt" },
  });
  eq(describeRes.isError, false, "describe_aggregate isError=false");
  contains(describeRes.content[0].text, '"name": "RestartPrompt"', "describe_aggregate names RestartPrompt");

  // -- describe_aggregate on a missing name surfaces the available list
  const describeMissRes = await client.callTool({
    name: "storehouse__describe_aggregate",
    arguments: { aggregates_dir: SMOKE_ROOT, aggregate_name: "RestartPrompt::DoesNotExist" },
  });
  eq(describeMissRes.isError, true, "describe_aggregate miss isError=true");
  contains(describeMissRes.content[0].text, "available", "describe_aggregate miss lists available names");

  // -- storehouse__list_aggregates returns RestartPrompt
  const listAggRes = await client.callTool({
    name: "storehouse__list_aggregates",
    arguments: { aggregates_dir: SMOKE_ROOT },
  });
  eq(listAggRes.isError, false, "list_aggregates isError=false");
  contains(listAggRes.content[0].text, "RestartPrompt", "list_aggregates mentions RestartPrompt");

  // -- storehouse__query rejects a PascalCase verb (command-shape)
  const queryShapeRes = await client.callTool({
    name: "storehouse__query",
    arguments: {
      aggregates_dir: SMOKE_ROOT,
      verb: "Tools::ShellTool.Bash",
      summary: "intentionally wrong shape to test query verb guard",
    },
  });
  eq(queryShapeRes.isError, true, "query rejects command-shape verb");
  contains(queryShapeRes.content[0].text, "looks like a command", "query shape error mentions command");

  // -- storehouse__dispatch without summary must be rejected (i606)
  const dispatchNoSummaryRes = await client.callTool({
    name: "storehouse__dispatch",
    arguments: {
      aggregates_dir: SMOKE_ROOT,
      command: "Tools::ShellTool.Bash",
      args: { id: `cli-smoke-no-summary-${RUN_ID}`, shell_command: "echo nope" },
      summary: "",
    },
  });
  eq(dispatchNoSummaryRes.isError, true, "dispatch without summary isError=true");
  contains(dispatchNoSummaryRes.content[0].text, "summary is required", "dispatch no-summary error mentions 'summary is required'");

  // -- storehouse__dispatch — universal door. Dispatch ShellTool.Bash
  // and assert ok=true + state carries the captured shell command.
  const SMOKE_BASH_ID = `cli-smoke-bash-${RUN_ID}`;
  const dispatchRes = await client.callTool({
    name: "storehouse__dispatch",
    arguments: {
      aggregates_dir: SMOKE_ROOT,
      command: "Tools::ShellTool.Bash",
      args: { id: SMOKE_BASH_ID, shell_command: "echo smoke", description: "cli smoke" },
      summary: "cli smoke dispatch for storehouse-mcp test",
    },
  });
  eq(dispatchRes.isError, false, "dispatch ShellTool.Bash isError=false");
  // REWRITTEN for hecksagain (migration plan Part 5, dispatch.mjs's own
  // rewrite -- see its header): the i654/i697-era rich timeline/
  // invocation-id/"exit N" rendering scraped the retired Rust binary's
  // human-readable stdout ; hecksagain-cli emits clean {ok, state,
  // events} JSON directly and dispatch.mjs's render() renders exactly
  // that shape (headline, an "Events:" section listing each event's
  // name + aggregate#id, a "State:" section with the record's JSON) --
  // no timeline text, no per-tool "exit N" line, no "Aggregate.Command#"
  // invocation marker. These assertions were stale against the CURRENT
  // dispatch.mjs (found live running the real MCP surface, not by
  // reading code) ; updated to check the rendering that actually exists.
  const dispatchText = dispatchRes.content[0].text;
  contains(dispatchText, "✓ Tools::ShellTool.Bash", "dispatch text has success headline");
  contains(dispatchText, "Events:", "dispatch text has an Events section");
  contains(dispatchText, "BashRan", "dispatch events section names the emitted event");
  contains(dispatchText, `Tools::ShellTool#${SMOKE_BASH_ID}`, "dispatch events section carries the aggregate#id");
  contains(dispatchText, "State:", "dispatch text has a State section");

  // -- multi-line output renders as real newlines, not escaped \\n (i607)
  const multiLineRes = await client.callTool({
    name: "storehouse__dispatch",
    arguments: {
      aggregates_dir: SMOKE_ROOT,
      command: "Tools::ShellTool.Bash",
      args: { id: `cli-smoke-multiline-${RUN_ID}`, shell_command: "printf 'line1\\nline2\\nline3\\n'", description: "multiline test" },
      summary: "verify multi-line stdout renders without escape sequences",
    },
  });
  const multiLineText = multiLineRes.content[0].text;
  const realNewlines = (multiLineText.match(/\n/g) || []).length;
  const escapedNewlines = (multiLineText.match(/\\n/g) || []).length;
  truthy(realNewlines > escapedNewlines, "multi-line output has real newlines, not escaped \\n");

  // -- storehouse__state — reads back the record storehouse__dispatch just
  // wrote, by its fully-qualified Domain::Aggregate name (hecksagain-cli's
  // `state` subcommand splits the FQN on "::" -- a bare aggregate name like
  // the old Rust-CLI-era "ShellTool" no longer resolves). Tools::ShellTool
  // now durably persists (see RUN_ID's own comment above) so this reads as
  // a genuine hit, not a "does not outlive its process" miss the way an
  // earlier version of this test expected under a since-changed corpus
  // binding.
  const stateRes = await client.callTool({
    name: "storehouse__state",
    arguments: {
      aggregates_dir: SMOKE_ROOT,
      aggregate_name: "Tools::ShellTool",
      id: SMOKE_BASH_ID,
      summary: "verify the ShellTool record dispatch just wrote reads back correctly",
    },
  });
  eq(stateRes.isError, false, "state read isError=false");
  contains(stateRes.content[0].text, '"ok":true', "state read finds the just-dispatched record");
  contains(stateRes.content[0].text, '"aggregate":"Tools::ShellTool"', "state answer names the FQN aggregate asked for");

  // -- storehouse__state on a missing id returns ok=false (not error).
  const stateMissRes = await client.callTool({
    name: "storehouse__state",
    arguments: {
      aggregates_dir: SMOKE_ROOT,
      aggregate_name: "Tools::ShellTool",
      id: `no-such-id-cli-smoke-${RUN_ID}`,
      summary: "confirm missing id returns ok=false gracefully",
    },
  });
  eq(stateMissRes.isError, false, "state miss isError=false (ok answer)");
  contains(stateMissRes.content[0].text, '"ok":false', "state miss ok=false");

  // -- storehouse__macrophage_check on a non-bluebook surfaces a complaint.
  // aggregates_dir is a real, new-under-this-rewrite required param (the
  // Macrophage domain needs to know WHERE to dispatch its Record*Edit
  // command against) -- see macrophage_check.mjs's own header.
  const macroRes = await client.callTool({
    name: "storehouse__macrophage_check",
    arguments: { aggregates_dir: SMOKE_ROOT, bluebook_path: "/tmp/not-a-bluebook.rs" },
  });
  truthy(macroRes.content[0].text.length > 0, "macrophage_check returned output");

  // -- storehouse__behaviors -- NOT YET SUPPORTED under hecksagain (Part 1
  // item 5). Confirms the tool answers honestly (isError, names the gap)
  // instead of crashing or silently no-opping, rather than asserting a
  // 'passed' count that no interpreter exists to produce.
  if (existsSync(SMOKE_BEHAVIORS)) {
    const behaviorsRes = await client.callTool({
      name: "storehouse__behaviors",
      arguments: { behaviors_path: SMOKE_BEHAVIORS },
    });
    eq(behaviorsRes.isError, true, "behaviors isError=true (not yet supported)");
    contains(behaviorsRes.content[0].text, "supported", "behaviors output names the unsupported gap");
  } else {
    process.stderr.write(`skip behaviors (no restart_prompt.behaviors)\n`);
  }

  // -- storehouse__conceive_behaviors -- same NOT YET SUPPORTED contract.
  const conceiveBehaviorsRes = await client.callTool({
    name: "storehouse__conceive_behaviors",
    arguments: { bluebook_path: SMOKE_ROOT },
  });
  eq(conceiveBehaviorsRes.isError, true, "conceive_behaviors isError=true (not yet supported)");
  contains(conceiveBehaviorsRes.content[0].text, "supported", "conceive_behaviors output names the unsupported gap");

  // -- events resource read returns the file contents
  const eventsRead = await client.readResource({ uri: "storehouse://events" });
  eq(eventsRead.contents[0].uri, "storehouse://events", "events read uri");
  eq(eventsRead.contents[0].text, "", "events file initially empty");

  // -- subscribe + write a line → expect a notifications/resources/updated
  let notifyFired = false;
  client.setNotificationHandler(
    (await import("@modelcontextprotocol/sdk/types.js")).ResourceUpdatedNotificationSchema,
    async (msg) => {
      if (msg.params?.uri === "storehouse://events") notifyFired = true;
    },
  );
  await client.subscribeResource({ uri: "storehouse://events" });

  // Give the watcher a beat to attach, then append a synthetic event.
  await new Promise((r) => setTimeout(r, 200));
  appendFileSync(eventsPath, JSON.stringify({ kind: "smoke", at: Date.now() }) + "\n");
  // Wait up to 2s for notification.
  for (let i = 0; i < 20 && !notifyFired; i++) {
    await new Promise((r) => setTimeout(r, 100));
  }
  truthy(notifyFired, "events subscribe → resources/updated notification fired");

  // After the notification fires, a re-read should surface the new line.
  const eventsRead2 = await client.readResource({ uri: "storehouse://events" });
  contains(eventsRead2.contents[0].text, '"kind":"smoke"', "events re-read includes new line");

  await client.close();
  try { unlinkSync(eventsPath); } catch {}

  if (failed > 0) {
    process.stderr.write(`\n${failed} smoke check(s) FAILED\n`);
    process.exit(1);
  }
  process.stderr.write("\nall cli smoke checks passed\n");
}

main().catch((err) => {
  process.stderr.write(`cli smoke fatal: ${err.stack || err}\n`);
  process.exit(1);
});
