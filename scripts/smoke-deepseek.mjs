import assert from "node:assert/strict";
import { mkdtemp, readFile, rm } from "node:fs/promises";
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
import { sourceState } from "./source-state.mjs";
import { fileURLToPath } from "node:url";

const apiKey =
  process.env.DEEPSEEK_API_KEY ??
  (process.env.RSS_DEEPSEEK_KEY_FILE
    ? (await readFile(process.env.RSS_DEEPSEEK_KEY_FILE, "utf8")).trim()
    : undefined);
if (!apiKey)
  throw Error(
    "Official smoke requires DEEPSEEK_API_KEY or RSS_DEEPSEEK_KEY_FILE; no fixture fallback",
  );
const root = fileURLToPath(new URL("../", import.meta.url)),
  start = sourceState(root);
const directory = await mkdtemp(join(tmpdir(), "rss-deepseek-official-"));
const config = {
  namespace: {
    tenantId: "smoke-tenant",
    principalId: "smoke-user",
    authorityId: "smoke-authority",
    sessionId: "smoke-session",
  },
  provider: "deepseek",
  config: { id: "official-deepseek", revision: "1" },
  accountRef: "smoke-account",
  workingDirectory: directory,
  permissions: "tools_disabled",
};
const budget = (timeoutMs = 120000) => ({
  timeoutMs,
  signal: new AbortController().signal,
});
const options = {
  resolveConfiguration: async () => ({
    configuration: config,
    persistenceDirectory: directory,
    apiKey,
    model: process.env.DEEPSEEK_MODEL ?? "deepseek-chat",
  }),
};
const ports = [];
const port = () => {
  const p = createDeepSeekAdapter(options);
  ports.push(p);
  return p;
};
const prompt = (id, text) => ({
  schemaVersion: 2,
  kind: "command",
  sessionId: config.namespace.sessionId,
  commandId: id,
  expiresAtMs: Date.now() + 180000,
  input: { type: "prompt", policy: "queue_next", text },
});
async function turn(p, b, c) {
  const attempt = fixtureAttempt(b, c),
    sent = await p.submit(b, c, attempt, budget());
  assert.equal(sent.certainty, "submitted");
  const events = [];
  for await (const e of p.observe(sent.binding, budget())) events.push(e);
  assert.equal(
    events.at(-1)?.body?.outcome,
    "completed",
    "official turn must complete",
  );
  assert.ok(
    events.some((e) => e.type === "delta"),
    "official native incremental text required",
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
  await first.close(budget(10000));
  const previous = {
    schemaVersion: 2,
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
    schemaVersion: 2,
    kind: "commandRecord",
    command,
    receipt: {
      schemaVersion: 2,
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
  await second.close(budget(10000));
  let proposals = 0,
    questions = 0;
  const controlled = {
    ...config,
    namespace: { ...config.namespace, sessionId: "controlled-smoke" },
    config: { id: "official-controlled", revision: "1" },
    permissions: "host_mediated",
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
            verificationRef: "official-smoke-test-verifier",
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
    resolveConfiguration: async () => ({
      configuration: controlled,
      persistenceDirectory: directory,
      apiKey,
      model: process.env.DEEPSEEK_MODEL ?? "deepseek-chat",
    }),
  });
  ports.push(third);
  const checked = unwrap(
    await VerifiedProviderSession.open(third, controlled, budget(15000)),
  );
  const request = {
    ...prompt(
      "controlled",
      'First call ask_user_question with one question id q, text "Run a test probe?", options yes/no. After the user answers yes, call host_propose exactly once with name test_probe and arguments {}. Finally reply only done. These are test tools, not device operations.',
    ),
    sessionId: controlled.namespace.sessionId,
  };
  const submitted = await third.submit(
    checked.binding,
    request,
    fixtureAttempt(checked.binding, request),
    budget(),
  );
  assert.equal(submitted.certainty, "submitted");
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
      unwrap(
        await third.respond(
          event.binding,
          {
            ...prompt("answer", ""),
            sessionId: controlled.namespace.sessionId,
            input: {
              type: "respond",
              interactionId: event.interaction.interactionId,
              generation: event.binding.generation,
              answer,
            },
          },
          budget(),
        ),
      );
    }
    if (event.body?.type === "terminal") terminal = event.body.outcome;
  }
  assert.equal(terminal, "completed");
  assert.ok(questions >= 1);
  assert.equal(proposals, 1);
  console.log(
    JSON.stringify({
      evidence: "official_deepseek",
      endpoint: "https://api.deepseek.com",
      model: process.env.DEEPSEEK_MODEL ?? "deepseek-chat",
      platform: process.platform,
      arch: process.arch,
      node: process.version,
      source: start,
      lockSha256: createHash("sha256")
        .update(await readFile(new URL("../pnpm-lock.yaml", import.meta.url)))
        .digest("hex"),
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
    }),
  );
} finally {
  for (const p of ports) unwrap(await p.close(budget(10000)));
  await rm(directory, { recursive: true, force: true });
}
