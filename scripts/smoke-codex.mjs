import {
  startStage,
  providerStage,
} from "../packages/ai-contract/dist/index.js";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import {
  chmodSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import {
  CODEX_VERSION,
  createCodexAdapter,
} from "../packages/ai-adapters/codex/dist/index.js";
import { VerifiedProviderSession } from "../packages/ai-contract/dist/session.js";
import {
  startSmokeModelGateway,
  continuityChallenge,
} from "./codex-smoke-model.mjs";
import { sameCommittedSource, sourceState } from "./source-state.mjs";

const allowedStages = [
  "configuration",
  "backend_identity",
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

export function describeFailure(stage, detail = {}) {
  const output = {
    stage: allowedStages.includes(stage) ? stage : "unknown",
  };
  if (["submitted", "not_sent", "unknown"].includes(detail.certainty))
    output.certainty = detail.certainty;
  if (["completed", "cancelled", "failed", "unknown"].includes(detail.outcome))
    output.outcome = detail.outcome;
  if (
    [
      "invalid_input",
      "permission_denied",
      "stale_binding",
      "unsupported_capability",
      "unsupported_version",
      "unavailable",
      "limit_exceeded",
    ].includes(detail.code)
  )
    output.code = detail.code;
  if (["never", "same_command", "reconcile_first"].includes(detail.retry))
    output.retry = detail.retry;
  return output;
}

export function loadSmokeConfiguration(args, env) {
  if (args.length && !(args.length === 2 && args[0] === "--config-file"))
    throw new Error("invalid smoke configuration");
  let value;
  try {
    value = args.length
      ? JSON.parse(readFileSync(args[1], "utf8"))
      : {
          mode: env.RSS_CODEX_SMOKE_MODE,
          apiUrl: env.RSS_CODEX_SMOKE_API_URL,
          apiKey: env.RSS_CODEX_SMOKE_API_KEY,
          model: env.RSS_CODEX_SMOKE_MODEL,
        };
  } catch {
    throw new Error("invalid smoke configuration");
  }
  if (
    !value ||
    typeof value !== "object" ||
    !["real_model", "local_fixture"].includes(value.mode) ||
    typeof value.apiUrl !== "string" ||
    typeof value.apiKey !== "string" ||
    typeof value.model !== "string" ||
    !value.apiKey ||
    !value.model
  )
    throw new Error("invalid smoke configuration");
  let url;
  try {
    url = new URL(value.apiUrl);
  } catch {
    throw new Error("invalid smoke configuration");
  }
  const loopback = ["localhost", "127.0.0.1", "[::1]"].includes(url.hostname);
  if (
    url.username ||
    url.password ||
    url.search ||
    url.hash ||
    !url.pathname.replace(/\/$/, "").endsWith("/v1") ||
    (value.mode === "real_model" && url.protocol !== "https:") ||
    (value.mode === "local_fixture" && !loopback) ||
    (url.protocol !== "https:" && url.protocol !== "http:")
  )
    throw new Error("invalid smoke configuration");
  const apiUrl = url.href.replace(/\/$/, "");
  return {
    mode: value.mode,
    apiUrl,
    apiKey: value.apiKey,
    model: value.model,
    endpoint: {
      endpointSha256: createHash("sha256").update(apiUrl).digest("hex"),
      mode: value.mode,
    },
  };
}

export async function closeAdapters(adapters, directory) {
  let processesStopped = true;
  const attempts = [];
  for (const adapter of adapters) {
    let stopped = false;
    let count = 0;
    while (count < 2 && !stopped) {
      count++;
      try {
        const result = await adapter.close({
          timeoutMs: 15000,
          signal: AbortSignal.timeout(15000),
        });
        stopped = result.ok && result.value.processStopped === true;
      } catch {
        // Continue closing every native incarnation without exposing provider diagnostics.
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

async function main() {
  const root = fileURLToPath(new URL("../", import.meta.url));
  const start = sourceState(root);
  let configured = false;
  let behaviorPassed = false;
  let processesStopped = true;
  let cleanup = {
    processesStopped: true,
    attempts: [],
    directoryRemoved: true,
  };
  let failure;
  let settings;
  let directory;
  let gateway;
  let backendIdentityVerified = false;
  let stage = "configuration";
  let detail = {};
  const results = [];
  const adapters = [];
  try {
    settings = loadSmokeConfiguration(process.argv.slice(2), process.env);
    configured = true;
    stage = "backend_identity";
    if (settings.mode === "real_model")
      gateway = await startSmokeModelGateway(settings);
    const challenge = continuityChallenge();
    directory = realpathSync(mkdtempSync(join(tmpdir(), "rss-codex-smoke-")));
    const nativeDirectory = join(directory, "native");
    const workingDirectory = join(directory, "project");
    mkdirSync(nativeDirectory, { mode: 0o700 });
    chmodSync(nativeDirectory, 0o700);
    mkdirSync(workingDirectory);
    const configuration = {
      provider: "codex",
      config: { id: "model-smoke", revision: "1" },

      workingDirectory,
      namespace: {
        tenantId: "model-smoke",
        principalId: "model-smoke",
        authorityId: "model-smoke",
        sessionId: "smoke-session",
      },
      permissions: "tools_disabled",
    };
    const create = () =>
      createCodexAdapter({
        resolveConfiguration: async (identity) => ({
          configuration,
          nativeDirectory,
          authentication: {
            type: "api_key",
            apiUrl: gateway?.apiUrl ?? settings.apiUrl,
            apiKey: gateway?.apiKey ?? settings.apiKey,
          },
          model: settings.model,
          ...(identity.history
            ? {
                ownedHistory: {
                  nativeSessionId: identity.history.nativeSessionId,
                  nativeThreadId: identity.history.nativeThreadId,
                },
              }
            : {}),
        }),
      }).agent;
    const budget = (timeoutMs = 90000) => ({
      timeoutMs,
      signal: AbortSignal.timeout(timeoutMs),
    });
    const requireResult = (result) => {
      detail = result.ok ? {} : result.error;
      assert.equal(result.ok, true);
      return result.value;
    };
    async function run(adapter, binding, id, text, expected) {
      stage = "submit";
      detail = {};
      const attemptId = `attempt-${id}`;
      const sent = await adapter.dispatch(
        binding,
        {
          schemaVersion: 5,
          kind: "command",
          sessionId: configuration.namespace.sessionId,
          commandId: id,
          expiresAtMs: Date.now() + 120000,
          input: { type: "prompt", text, policy: "queue_next" },
        },
        {
          attemptId,
          originGeneration: binding.generation,
          observerGeneration: binding.generation,
          nativeSessionId: binding.nativeSessionId,
          nativeThreadId: binding.nativeThreadId,
          certainty: "intent",
        },
        budget(),
      );
      detail = { certainty: sent.certainty, ...sent.error };
      assert.equal(sent.certainty, "submitted");
      stage = "observe";
      detail = {};
      const observations = [];
      for await (const observation of adapter.observe(sent.binding, budget())) {
        observations.push(observation);
        if (observation.body?.type === "terminal") break;
      }
      stage = "terminal";
      detail = { outcome: observations.at(-1)?.body?.outcome };
      assert.equal(detail.outcome, "completed");
      stage = "text";
      assert.equal(
        observations
          .filter((observation) => observation.body?.type === "text")
          .map((observation) => observation.body.text)
          .join("")
          .trim(),
        expected,
      );
      results.push({
        scenario: id,
        submitted: true,
        nativeTerminal: "completed",
        textMatched: true,
        deltas: observations.filter(
          (observation) => observation.type === "delta",
        ).length,
      });
      return sent.binding;
    }
    stage = "open";
    const adapter = create();
    adapters.push(adapter);
    const opened = requireResult(
      await adapter.createSession(configuration, budget(30000)),
    );
    const first = await run(
      adapter,
      opened.binding,
      "new-session",
      challenge.prompts[0],
      challenge.expected,
    );
    const second = await run(
      adapter,
      first,
      "same-process",
      challenge.prompts[1],
      challenge.expected,
    );
    stage = "close";
    assert.equal(
      requireResult(await adapter.close(budget(15000))).processStopped,
      true,
    );
    const resumed = create();
    adapters.push(resumed);
    stage = "resume";
    const restored = requireResult(
      await VerifiedProviderSession.restore(
        resumed,
        startStage(
          {
            schemaVersion: 5,
            kind: "session",
            namespace: configuration.namespace,
            revision: 0,
            lastSequence: 0,
            status: "active",
            stages: [],
          },
          providerStage(second, opened.capabilities),
        ),
        configuration,
        budget(30000),
      ),
    );
    stage = "resume_identity";
    assert.equal(restored.binding.nativeSessionId, second.nativeSessionId);
    assert.equal(restored.binding.nativeThreadId, second.nativeThreadId);
    assert.notEqual(restored.binding.generation, second.generation);
    await run(
      resumed,
      restored.binding,
      "cold-resume",
      challenge.prompts[2],
      challenge.expected,
    );
    behaviorPassed = true;
    stage = "backend_identity";
    if (settings.mode === "real_model") {
      backendIdentityVerified = await gateway.verify();
      assert.equal(backendIdentityVerified, true);
    }
  } catch {
    failure = describeFailure(stage, detail);
    process.exitCode = 1;
  } finally {
    if (directory) {
      cleanup = await closeAdapters(adapters, directory);
      processesStopped = cleanup.processesStopped;
      if (!processesStopped && !failure) failure = describeFailure("cleanup");
    }
    await gateway?.close();
    const end = sourceState(root);
    const deliverable =
      configured &&
      behaviorPassed &&
      (settings.mode === "local_fixture" || backendIdentityVerified) &&
      processesStopped &&
      sameCommittedSource(start, end);
    if (!deliverable) process.exitCode = 1;
    const adapterPackage = join(
      root,
      "packages/ai-adapters/codex/package.json",
    );
    const protocolManifest = join(
      root,
      "packages/ai-adapters/codex/protocol-manifest.json",
    );
    const evidence = {
      evidence:
        settings?.mode === "local_fixture"
          ? "real-codex-local-responses-fixture-smoke"
          : "real-codex-configured-model-smoke",
      status: !configured ? "not_run" : deliverable ? "passed" : "failed",
      configured,
      behaviorPassed,
      processesStopped,
      cleanup,
      ...(settings
        ? {
            endpoint: settings.endpoint,
            requestedModelSha256: createHash("sha256")
              .update(settings.model)
              .digest("hex"),
          }
        : {}),
      backendIdentityVerified,
      backendReceipts: gateway?.receipts ?? [],
      source: { start, end },
      lockSha256: createHash("sha256")
        .update(readFileSync(join(root, "pnpm-lock.yaml")))
        .digest("hex"),
      packageIdentitySha256: createHash("sha256")
        .update(readFileSync(adapterPackage))
        .update(readFileSync(protocolManifest))
        .digest("hex"),
      codex: CODEX_VERSION,
      platform: process.platform,
      arch: process.arch,
      node: process.version,
      timestamp: new Date().toISOString(),
      results,
      failure,
      notCovered: [
        ...(backendIdentityVerified ? [] : ["real model/backend identity"]),
        "operating-system containment beyond this platform run",
        "business execution",
        "full product assembly",
      ],
    };
    mkdirSync(join(root, ".local-ci-runs"), { recursive: true });
    writeFileSync(
      join(root, ".local-ci-runs/codex-smoke.json"),
      JSON.stringify(evidence, null, 2),
    );
    console.log(
      JSON.stringify({
        status: evidence.status,
        mode: settings?.mode ?? "not_configured",
        behaviorPassed,
        processesStopped,
        failure,
        results,
      }),
    );
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href)
  await main();
