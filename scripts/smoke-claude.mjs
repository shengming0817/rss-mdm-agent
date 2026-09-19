import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import {
  mkdtempSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
  rmSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import {
  createClaudeAdapter,
  SDK_VERSION,
  CLI_VERSION,
} from "../packages/ai-adapters/claude/dist/index.js";
import { sourceState, sameCommittedSource } from "./source-state.mjs";
const root = fileURLToPath(new URL("../", import.meta.url)),
  start = sourceState(root);
// Optional explicitly selected file is read only for credential/URL fields. No user settings enter the child.
const args = process.argv.slice(2);
if (args.length && !(args.length === 2 && args[0] === "--credential-file"))
  throw new Error("Expected --credential-file PATH");
const credentials = args.length
  ? JSON.parse(readFileSync(args[1], "utf8")).env
  : process.env;
const key = credentials.ANTHROPIC_API_KEY,
  token = credentials.ANTHROPIC_AUTH_TOKEN,
  url = credentials.ANTHROPIC_BASE_URL;
if (!url || !!key === !!token)
  throw new Error("Set API URL and exactly one API key or auth token");
const directory = mkdtempSync(join(tmpdir(), "rss-claude-model-"));
mkdirSync(join(directory, "config"), { mode: 0o700 });
mkdirSync(join(directory, "project"));
const configuration = {
  provider: "claude",
  config: { id: "model-smoke", revision: "1" },
  accountRef: "model-smoke",
  workingDirectory: join(directory, "project"),
  permissions: "tools_disabled",
};
const create = () =>
  createClaudeAdapter({
    resolveConfiguration: async () => ({
      configuration,
      configurationDirectory: join(directory, "config"),
      apiUrl: url,
      credential: key
        ? { type: "api_key", value: key }
        : { type: "auth_token", value: token },
      ...(credentials.ANTHROPIC_MODEL
        ? { model: credentials.ANTHROPIC_MODEL }
        : {}),
    }),
  });
const budget = (ms = 90000) => ({
    timeoutMs: ms,
    signal: AbortSignal.timeout(ms),
  }),
  adapters = [];
let passed = false,
  failure;
const results = [];
async function run(adapter, binding, id, text, expected) {
  const sent = await adapter.submit(
    binding,
    {
      schemaVersion: 2,
      kind: "command",
      sessionId: "smoke-session",
      commandId: id,
      expiresAtMs: Date.now() + 120000,
      input: { type: "prompt", text, policy: "queue_next" },
    },
    budget(),
  );
  assert.equal(sent.certainty, "submitted");
  const observations = await Array.fromAsync(
    adapter.observe(sent.binding, budget()),
  );
  assert.equal(observations.at(-1)?.body?.outcome, "completed");
  assert.ok(
    observations.some(
      (o) => o.body?.type === "text" && o.body.text.includes(expected),
    ),
  );
  results.push({
    scenario: id,
    submitted: true,
    nativeTerminal: "completed",
    textMatched: true,
    deltas: observations.filter((o) => o.type === "delta").length,
  });
  return sent.binding;
}
try {
  const adapter = create();
  adapters.push(adapter);
  const opened = await adapter.createSession(configuration, budget(30000));
  assert.equal(opened.ok, true);
  const first = await run(
    adapter,
    opened.value.binding,
    "new-session",
    "Remember the test code RSS_SDK_2406. Reply with exactly RSS_SDK_2406.",
    "RSS_SDK_2406",
  );
  const second = await run(
    adapter,
    first,
    "same-process",
    "What is the test code I asked you to remember? Reply with only that code.",
    "RSS_SDK_2406",
  );
  assert.equal(
    (await adapter.close(budget(15000))).value?.processStopped,
    true,
  );
  const resumed = create();
  adapters.push(resumed);
  const rebound = await resumed.resume(second, budget(30000));
  assert.equal(rebound.ok, true);
  assert.equal(rebound.value.nativeSessionId, second.nativeSessionId);
  assert.notEqual(rebound.value.generation, second.generation);
  await run(
    resumed,
    rebound.value,
    "cold-resume",
    "What is the test code I asked you to remember? Reply with only that code.",
    "RSS_SDK_2406",
  );
  passed = true;
} catch {
  failure = "model smoke failed";
  process.exitCode = 1;
} finally {
  let processesStopped = true;
  for (const adapter of adapters) {
    const result = await adapter.close(budget(15000));
    processesStopped &&= result.ok && result.value.processStopped;
  }
  rmSync(directory, { recursive: true, force: true });
  const end = sourceState(root),
    deliverable = passed && processesStopped && sameCommittedSource(start, end);
  if (!deliverable) process.exitCode = 1;
  const evidence = {
    evidence: "real-sdk-real-model",
    status: deliverable ? "passed" : "failed",
    behaviorPassed: passed,
    processesStopped,
    source: { start, end },
    sdk: SDK_VERSION,
    cli: CLI_VERSION,
    lockSha256: createHash("sha256")
      .update(readFileSync(join(root, "pnpm-lock.yaml")))
      .digest("hex"),
    platform: process.platform,
    arch: process.arch,
    node: process.version,
    credentialMode: key ? "api_key" : "auth_token",
    timestamp: new Date().toISOString(),
    results,
    failure,
    notCovered: [
      "Windows containment",
      "business execution",
      "full product assembly",
    ],
  };
  mkdirSync(join(root, ".local-ci-runs"), { recursive: true });
  writeFileSync(
    join(root, ".local-ci-runs/claude-model.json"),
    JSON.stringify(evidence, null, 2),
  );
  console.log(
    JSON.stringify({
      status: evidence.status,
      behaviorPassed: passed,
      processesStopped,
      results,
    }),
  );
}
