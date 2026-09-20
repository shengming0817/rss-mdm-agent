import assert from "node:assert/strict";
import { mkdtemp, readFile, mkdir, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createDeepSeekAdapter } from "../packages/ai-adapters/deepseek/dist/index.js";
import { VerifiedProviderSession } from "../packages/ai-contract/dist/session.js";
import { fingerprint } from "../packages/ai-contract/dist/index.js";
import {
  fixtureLimits,
  fixtureAttempt,
  unwrap,
} from "../packages/ai-contract/dist/testing/index.js";
import { createHash } from "node:crypto";
import { sourceState, sameCommittedSource } from "./source-state.mjs";
import { fileURLToPath, pathToFileURL } from "node:url";

/** Snapshot the selected endpoint once; a configured endpoint is not backend identity proof. */
export function smokeConfiguration(env) {
  try {
    const url = new URL(env.DEEPSEEK_BASE_URL ?? "https://api.deepseek.com");
    const loopback = ["localhost", "127.0.0.1", "[::1]"].includes(url.hostname);
    const model = env.DEEPSEEK_MODEL ?? "deepseek-chat";
    if (
      (url.protocol !== "https:" && !(url.protocol === "http:" && loopback)) ||
      url.username ||
      url.password ||
      url.search ||
      url.hash ||
      typeof model !== "string" ||
      !model.trim()
    )
      throw new Error();
    const apiUrl = url.href.replace(/\/$/, "");
    return Object.freeze({
      apiUrl,
      model,
      endpoint: Object.freeze({
        kind: loopback
          ? "local_fixture"
          : url.origin === "https://api.deepseek.com"
            ? "official"
            : "configured",
        endpointSha256: createHash("sha256").update(apiUrl).digest("hex"),
      }),
    });
  } catch {
    throw new Error("Invalid smoke configuration");
  }
}

export async function closeAdapters(ports, directory) {
  const attempts = [];
  let processesStopped = true,
    directoryRemoved = false;
  for (const port of ports) {
    let stopped = false,
      count = 0;
    while (count < 2 && !stopped) {
      count++;
      try {
        const result = await port.close({
          timeoutMs: 10000,
          signal: AbortSignal.timeout(10000),
        });
        stopped = result.ok && result.value.processStopped === true;
      } catch {
        /* Continue closing other owned processes. */
      }
    }
    attempts.push(count);
    processesStopped &&= stopped;
  }
  if (processesStopped) {
    try {
      await rm(directory, { recursive: true, force: true });
      directoryRemoved = true;
    } catch {}
  }
  return { processesStopped, directoryRemoved, attempts };
}
export function deliverable(
  behaviorPassed,
  cleanup,
  start,
  end,
  lockStart,
  lockEnd,
) {
  return (
    behaviorPassed &&
    cleanup.processesStopped &&
    cleanup.directoryRemoved &&
    sameCommittedSource(start, end) &&
    lockStart === lockEnd
  );
}
async function main() {
  const selected = smokeConfiguration(process.env);
  const apiKey =
    process.env.DEEPSEEK_API_KEY ??
    (process.env.RSS_DEEPSEEK_KEY_FILE
      ? (await readFile(process.env.RSS_DEEPSEEK_KEY_FILE, "utf8")).trim()
      : undefined);
  if (!apiKey)
    throw Error(
      "Smoke requires DEEPSEEK_API_KEY or RSS_DEEPSEEK_KEY_FILE; no fixture fallback",
    );
  const root = fileURLToPath(new URL("../", import.meta.url)),
    start = sourceState(root);
  const lockHash = async () =>
    createHash("sha256")
      .update(await readFile(new URL("../pnpm-lock.yaml", import.meta.url)))
      .digest("hex");
  const lockStart = await lockHash();
  const diagnostics = [];
  const diagnose = (value) => {
    if (diagnostics.length < 128) diagnostics.push(value);
  };
  let stage = "open",
    behaviorPassed = false,
    failure,
    results = {};
  const directory = await mkdtemp(join(tmpdir(), "rss-deepseek-smoke-"));
  const config = {
    namespace: {
      tenantId: "smoke-tenant",
      principalId: "smoke-user",
      authorityId: "smoke-authority",
      sessionId: "smoke-session",
    },
    provider: "deepseek",
    config: { id: "deepseek-smoke", revision: "1" },
    accountRef: "smoke-account",
    workingDirectory: directory,
    permissions: "tools_disabled",
  };
  const budget = (timeoutMs = 120000) => ({
    timeoutMs,
    signal: new AbortController().signal,
  });
  const options = {
    onDiagnostic: diagnose,
    resolveConfiguration: async () => ({
      configuration: config,
      persistenceDirectory: directory,
      apiUrl: selected.apiUrl,
      apiKey,
      model: selected.model,
    }),
  };
  const ports = [];
  const port = () => {
    const p = createDeepSeekAdapter(options);
    ports.push(p);
    return p;
  };
  const prompt = (id, text) => ({
    schemaVersion: 4,
    kind: "command",
    sessionId: config.namespace.sessionId,
    commandId: id,
    expiresAtMs: Date.now() + 180000,
    input: { type: "prompt", policy: "queue_next", text },
  });
  async function turn(p, b, c) {
    stage = "submit";
    const attempt = fixtureAttempt(b, c),
      sent = await p.dispatch(b, c, attempt, budget());
    assert.equal(sent.certainty, "submitted");
    stage = "observe";
    const events = [];
    for await (const e of p.observe(sent.binding, budget())) events.push(e);
    assert.equal(
      events.at(-1)?.body?.outcome,
      "completed",
      "configured turn must complete",
    );
    assert.ok(
      events.some((e) => e.type === "delta"),
      "configured native incremental text required",
    );
    return {
      attempt,
      sent,
      events,
      text: events
        .filter((e) => e.body?.type === "text")
        .map((e) => e.body.text)
        .join(""),
    };
  }
  try {
    const first = port(),
      admitted = unwrap(
        await VerifiedProviderSession.open(first, config, budget(15000)),
      );
    const command = prompt(
      "first",
      "Remember the code word AZURE_PEBBLE_71. Reply only: remembered.",
    );
    const initial = await turn(first, admitted.binding, command);
    stage = "close";
    assert.equal(unwrap(await first.close(budget(10000))).processStopped, true);
    stage = "restore";
    const previous = {
      schemaVersion: 4,
      kind: "session",
      namespace: config.namespace,
      revision: 0,
      lastSequence: 0,
      status: "active",
      binding: initial.sent.binding,
      capabilities: admitted.capabilities,
    };
    const second = port(),
      restored = unwrap(
        await VerifiedProviderSession.restore(
          second,
          previous,
          config,
          budget(15000),
        ),
      );
    const record = {
      schemaVersion: 4,
      kind: "commandRecord",
      command,
      receipt: {
        schemaVersion: 4,
        kind: "receipt",
        namespace: config.namespace,
        commandId: command.commandId,
        contentHash: fingerprint(command, fixtureLimits),
        acceptedAtMs: 0,
        retryUntilMs: command.expiresAtMs,
        receiptUntilMs: command.expiresAtMs,
        acceptedRevision: 1,
      },
      state: "reconciliation_required",
      dispatch: {
        ...initial.attempt,
        observerGeneration: restored.binding.generation,
        certainty: "submitted",
        nativeRequestId: initial.sent.binding.nativeRequestId,
      },
    };
    stage = "reconcile";
    const proof = unwrap(
      await restored.reconcile(
        { ...previous, binding: restored.binding },
        record,
        budget(),
      ),
    );
    assert.equal(proof.observation.status, "terminal");
    const resumed = await turn(
      second,
      restored.binding,
      prompt(
        "second",
        "What code word did I ask you to remember? Reply only the code word.",
      ),
    );
    assert.match(resumed.text, /AZURE_PEBBLE_71/);
    stage = "close";
    assert.equal(
      unwrap(await second.close(budget(10000))).processStopped,
      true,
    );
    stage = "controlled_open";
    let proposals = 0,
      questions = 0;
    const controlled = {
      ...config,
      namespace: { ...config.namespace, sessionId: "controlled-smoke" },
      config: { id: "controlled-smoke", revision: "1" },
      permissions: "host_mediated",
    };
    const admission = {
      // Test admission tied to the exact already exercised composition. This is
      // not the consuming product's platform verifier or an execution permit.
      verifier: {
        verify: async (session) => {
          assert.equal(
            session.binding.providerVersion,
            restored.binding.providerVersion,
          );
          assert.equal(session.capabilities.tools, "host_mediated");
          return {
            ok: true,
            value: {
              platform: process.platform,
              verificationRef: "smoke-test-verifier",
            },
          };
        },
      },
      tools: {
        propose: async (proposal) => {
          assert.equal(proposal.name, "test_probe");
          proposals++;
          return {
            ok: true,
            value: {
              disposition: "returned",
              text: "S1 test result; no device effect",
            },
          };
        },
      },
    };
    const third = createDeepSeekAdapter({
      tools: admission.tools,
      onDiagnostic: diagnose,
      resolveConfiguration: async () => ({
        configuration: controlled,
        persistenceDirectory: directory,
        apiUrl: selected.apiUrl,
        apiKey,
        model: selected.model,
      }),
    });
    ports.push(third);
    const checked = unwrap(
      await VerifiedProviderSession.open(
        third,
        controlled,
        budget(15000),
        admission,
      ),
    );
    const request = {
      ...prompt(
        "controlled",
        'First call ask_user_question with one question id q, text "Run a test probe?", options yes/no. After the user answers yes, call host_propose exactly once with name test_probe and arguments {}. Finally reply only done. These are test tools, not device operations.',
      ),
      sessionId: controlled.namespace.sessionId,
    };
    stage = "controlled_submit";
    const submitted = await third.dispatch(
      checked.binding,
      request,
      fixtureAttempt(checked.binding, request),
      budget(),
    );
    assert.equal(submitted.certainty, "submitted");
    stage = "controlled_observe";
    let terminal;
    for await (const event of third.observe(submitted.binding, budget())) {
      if (event.type === "interaction") {
        questions++;
        const answer = {
          answers: event.interaction.request.questions.map((q) => ({
            id: q.id,
            selected: q.options?.some((o) => o.label === "yes") ? ["yes"] : [],
            custom: "yes",
          })),
        };
        const response = {
          ...prompt("answer", ""),
          sessionId: controlled.namespace.sessionId,
          input: {
            type: "respond",
            interactionId: event.interaction.interactionId,
            generation: event.binding.generation,
            answer,
          },
        };
        const acknowledged = await third.dispatch(
          event.binding,
          response,
          fixtureAttempt(event.binding, response),
          budget(),
        );
        assert.equal(acknowledged.certainty, "acknowledged");
        assert.equal(acknowledged.acknowledgement.type, "respond");
      }
      if (event.body?.type === "terminal") terminal = event.body.outcome;
    }
    assert.equal(terminal, "completed");
    assert.ok(questions >= 1);
    assert.equal(proposals, 1);
    results = {
      bindingVersion: restored.binding.providerVersion,
      checks: {
        incremental: true,
        history: true,
        coldContextContinuation: true,
        structuredQuestion: true,
        controlledProposal: true,
      },
      deltas: [
        initial.events.filter((e) => e.type === "delta").length,
        resumed.events.filter((e) => e.type === "delta").length,
      ],
      questions,
      proposals,
    };
    behaviorPassed = true;
  } catch {
    failure = { stage };
  } finally {
    const cleanup = await closeAdapters(ports, directory);
    const end = sourceState(root),
      lockEnd = await lockHash();
    const passed = deliverable(
      behaviorPassed,
      cleanup,
      start,
      end,
      lockStart,
      lockEnd,
    );
    if (!passed) process.exitCode = 1;
    const evidence = {
      evidence:
        selected.endpoint.kind === "local_fixture"
          ? "real-deepseek-local-model-fixture"
          : "real-deepseek-configured-endpoint-smoke",
      status: passed ? "passed" : "failed",
      behaviorPassed,
      endpoint: selected.endpoint,
      model: selected.model,
      backendIdentityVerified: false,
      platform: process.platform,
      arch: process.arch,
      node: process.version,
      command: "pnpm smoke:deepseek",
      timestamp: new Date().toISOString(),
      source: { start, end },
      lockSha256: { start: lockStart, end: lockEnd },
      cleanup,
      failure,
      diagnostics,
      ...results,
      notCovered: [
        "Windows containment",
        "production platform verifier",
        "device or business execution",
        "full product assembly",
      ],
    };
    await mkdir(join(root, ".local-ci-runs"), { recursive: true });
    await writeFile(
      join(root, ".local-ci-runs/deepseek-model.json"),
      JSON.stringify(evidence, null, 2),
    );
    console.log(JSON.stringify(evidence));
  }
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href)
  await main();
