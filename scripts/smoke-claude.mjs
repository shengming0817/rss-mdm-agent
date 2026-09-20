import {
  startStage,
  providerStage,
} from "../packages/ai-contract/dist/index.js";
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
import { join, basename } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import {
  createClaudeAdapter,
  SDK_VERSION,
  CLI_VERSION,
} from "../packages/ai-adapters/claude/dist/index.js";
import { VerifiedProviderSession } from "../packages/ai-contract/dist/session.js";
import { sourceState, sameCommittedSource } from "./source-state.mjs";
const schema = JSON.parse(
  readFileSync(
    new URL(
      "../packages/ai-contract/schema/runtime.schema.json",
      import.meta.url,
    ),
    "utf8",
  ),
).$defs;
export function smokeCommand(id, text, expiresAtMs) {
  return {
    schemaVersion: schema.Command.properties.schemaVersion.const,
    kind: "command",
    sessionId: "smoke-session",
    commandId: id,
    expiresAtMs,
    input: { type: "prompt", text, policy: "queue_next" },
  };
}
export function smokeSession(namespace, binding, capabilities) {
  return startStage(
    {
      schemaVersion: schema.Session.properties.schemaVersion.const,
      kind: "session",
      namespace,
      revision: 0,
      lastSequence: 0,
      status: "active",
      stages: [],
    },
    providerStage(binding, capabilities),
  );
}
export function describeFailure(stage, detail = {}) {
  const stages = [
    "open",
    "submit",
    "observe",
    "terminal",
    "text",
    "close",
    "resume",
    "resume_identity",
    "cleanup",
  ];
  const output = { stage: stages.includes(stage) ? stage : "unknown" };
  const choices = {
    code: schema.ErrorCode.enum,
    retry: schema.Retry.enum,
    outcome: schema.Outcome.enum,
    certainty: ["submitted", "not_sent", "unknown"],
  };
  for (const [key, allowed] of Object.entries(choices))
    if (allowed.includes(detail[key])) output[key] = detail[key];
  return output;
}
export async function closeAdapters(adapters, directory) {
  let processesStopped = true;
  const attempts = [];
  for (const adapter of adapters) {
    let stopped = false,
      count = 0;
    for (; count < 2 && !stopped; ) {
      count++;
      try {
        const result = await adapter.close({
          timeoutMs: 15000,
          signal: AbortSignal.timeout(15000),
        });
        stopped = result.ok && result.value.processStopped === true;
      } catch {
        /* Do not leak provider errors or skip other adapters. */
      }
    }
    attempts.push(count);
    processesStopped &&= stopped;
  }
  if (processesStopped) rmSync(directory, { recursive: true, force: true });
  return {
    processesStopped,
    attempts,
    directoryRemoved: processesStopped,
    ...(!processesStopped ? { retainedDirectory: basename(directory) } : {}),
  };
}
export function smokeEndpoint(value) {
  try {
    const parsed = new URL(value);
    const loopback = ["localhost", "127.0.0.1", "[::1]"].includes(
      parsed.hostname,
    );
    if (
      parsed.username ||
      parsed.password ||
      parsed.search ||
      parsed.hash ||
      (parsed.protocol !== "https:" &&
        !(parsed.protocol === "http:" && loopback))
    )
      throw new Error();
    const apiUrl = parsed.href.replace(/\/$/, "");
    return {
      apiUrl,
      endpoint: {
        endpointSha256: createHash("sha256").update(apiUrl).digest("hex"),
        mode: loopback
          ? "loopback-compatible-endpoint"
          : parsed.hostname === "api.anthropic.com"
            ? "anthropic-api"
            : "configured-compatible-endpoint",
      },
    };
  } catch {
    throw new Error("Invalid endpoint configuration");
  }
}
async function main() {
  const root = fileURLToPath(new URL("../", import.meta.url)),
    start = sourceState(root);
  // Optional explicitly selected file is read only for credential/URL fields. No user settings enter the child.
  const args = process.argv.slice(2);
  if (args.length && !(args.length === 2 && args[0] === "--credential-file"))
    throw new Error("Expected --credential-file PATH");
  let credentials;
  try {
    credentials = args.length
      ? JSON.parse(readFileSync(args[1], "utf8")).env
      : process.env;
    if (!credentials || typeof credentials !== "object") throw new Error();
  } catch {
    // JSON parser and filesystem diagnostics can contain secret material or private paths.
    throw new Error("Invalid credential configuration");
  }
  const key = credentials.ANTHROPIC_API_KEY,
    token = credentials.ANTHROPIC_AUTH_TOKEN,
    url = credentials.ANTHROPIC_BASE_URL;
  if (!url || !!key === !!token)
    throw new Error("Set API URL and exactly one API key or auth token");
  const selected = smokeEndpoint(url);
  const directory = mkdtempSync(join(tmpdir(), "rss-claude-model-"));
  mkdirSync(join(directory, "config"), { mode: 0o700 });
  mkdirSync(join(directory, "project"));
  const configuration = {
    provider: "claude",
    config: { id: "model-smoke", revision: "1" },
    accountRef: "model-smoke",
    workingDirectory: join(directory, "project"),
    namespace: {
      tenantId: "model-smoke",
      principalId: "model-smoke",
      authorityId: "model-smoke",
      sessionId: "smoke-session",
    },
    permissions: "tools_disabled",
  };
  const create = () =>
    createClaudeAdapter({
      resolveConfiguration: async () => ({
        configuration,
        configurationDirectory: join(directory, "config"),
        apiUrl: selected.apiUrl,
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
    failure,
    stage = "open",
    detail = {};
  const requireResult = (result) => {
    detail = result.ok ? {} : result.error;
    assert.equal(result.ok, true);
    return result.value;
  };
  const results = [];
  async function run(adapter, binding, id, text, expected) {
    stage = "submit";
    detail = {};
    const sent = await adapter.dispatch(
      binding,
      smokeCommand(id, text, Date.now() + 120000),
      {
        attemptId: `attempt-${id}`,
        originGeneration: binding.generation,
        observerGeneration: binding.generation,
        nativeSessionId: binding.nativeSessionId,
        certainty: "intent",
      },
      budget(),
    );
    detail = { certainty: sent.certainty, ...sent.error };
    assert.equal(sent.certainty, "submitted");
    stage = "observe";
    detail = {};
    const observations = await Array.fromAsync(
      adapter.observe(sent.binding, budget()),
    );
    stage = "terminal";
    detail = { outcome: observations.at(-1)?.body?.outcome };
    assert.equal(detail.outcome, "completed");
    stage = "text";
    detail = {};
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
    requireResult(opened);
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
    stage = "close";
    detail = {};
    assert.equal(
      requireResult(await adapter.close(budget(15000))).processStopped,
      true,
    );
    const resumed = create();
    adapters.push(resumed);
    stage = "resume";
    detail = {};
    const rebound = await VerifiedProviderSession.restore(
      resumed,
      smokeSession(configuration.namespace, second, opened.value.capabilities),
      configuration,
      budget(30000),
    );
    requireResult(rebound);
    stage = "resume_identity";
    detail = {};
    assert.equal(rebound.value.binding.nativeSessionId, second.nativeSessionId);
    assert.notEqual(rebound.value.binding.generation, second.generation);
    await run(
      resumed,
      rebound.value.binding,
      "cold-resume",
      "What is the test code I asked you to remember? Reply with only that code.",
      "RSS_SDK_2406",
    );
    passed = true;
  } catch {
    failure = describeFailure(stage, detail);
    process.exitCode = 1;
  } finally {
    const cleanup = await closeAdapters(adapters, directory);
    const { processesStopped } = cleanup;
    if (!processesStopped && !failure) failure = describeFailure("cleanup");
    const end = sourceState(root),
      deliverable =
        passed && processesStopped && sameCommittedSource(start, end);
    if (!deliverable) process.exitCode = 1;
    const evidence = {
      evidence: "real-sdk-configured-endpoint-smoke",
      endpoint: selected.endpoint,
      requestedModel: credentials.ANTHROPIC_MODEL || "provider_default",
      backendIdentityVerified: false,
      status: deliverable ? "passed" : "failed",
      behaviorPassed: passed,
      processesStopped,
      cleanup,
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
        "upstream model/backend identity behind the configured endpoint",
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
        cleanup,
        failure,
        results,
      }),
    );
  }
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href)
  await main();
