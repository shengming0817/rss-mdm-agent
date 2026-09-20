import assert from "node:assert/strict";
import { test } from "node:test";
import { createRequire } from "node:module";
import { createAccessService } from "../../packages/ai-access/dist/index.js";
import { localTransportPair } from "../../packages/ai-client/dist/index.js";
import { fixtureCaller } from "../../packages/ai-contract/dist/testing/index.js";

const require = createRequire(
  new URL("../../packages/ai-access/package.json", import.meta.url),
);
const { client } = await import(require.resolve("@agentclientprotocol/sdk"));

const namespace = { ...fixtureCaller, sessionId: "session-1" };
const binding = {
  provider: "codex",
  providerVersion: "codex-0.155.0",
  adapterVersion: "0.1.0",
  generation: "generation-2",
  accountRef: "account",
  nativeSessionId: "native-session",
  nativeThreadId: "thread-1",
  nativeRunId: "run-1",
  nativeRequestId: "request-start",
  config: { id: "cfg", revision: "1" },
  workspaceId: "workspace",
};
const session = {
  schemaVersion: 4,
  kind: "session",
  namespace,
  revision: 1,
  lastSequence: 0,
  binding,
  capabilities: {
    continuation: "across_processes",
    cancellation: "request_only",
    tools: "disabled",
    steer: "supported",
    fork: "supported",
    subagent: "unsupported",
    terminal: "unsupported",
    structuredQuestion: "unsupported",
    multimodal: "unsupported",
  },
  status: "active",
};
const prompt = (commandId, policy, state, dispatch) => ({
  schemaVersion: 4,
  kind: "commandRecord",
  command: {
    schemaVersion: 4,
    kind: "command",
    sessionId: namespace.sessionId,
    commandId,
    expiresAtMs: 10_000,
    input: {
      type: "prompt",
      text: commandId,
      policy,
      ...(policy === "steer" ? { targetRunId: "run-1" } : {}),
    },
  },
  receipt: {
    schemaVersion: 4,
    kind: "receipt",
    namespace,
    commandId,
    contentHash: `hash-${commandId}`,
    acceptedAtMs: 0,
    retryUntilMs: 10_000,
    receiptUntilMs: 10_000,
    acceptedRevision: 1,
  },
  state,
  ...(dispatch ? { dispatch } : {}),
  ...(state === "terminal" ? { outcome: "completed" } : {}),
  ...(state === "invalidated"
    ? { failure: { code: "stale_binding", retry: "never" } }
    : {}),
});
const dispatch = (attemptId, nativeRequestId, overrides = {}) => ({
  attemptId,
  originGeneration: binding.generation,
  observerGeneration: binding.generation,
  nativeSessionId: binding.nativeSessionId,
  nativeThreadId: binding.nativeThreadId,
  nativeRunId: binding.nativeRunId,
  nativeRequestId,
  certainty: "submitted",
  ...overrides,
});

async function cancel(t, activeSession, records) {
  const cancellations = [];
  const host = {
    async snapshotPage() {
      return {
        ok: true,
        value: {
          session: activeSession,
          cursor: 0,
          events: [],
          commands: records,
          interactions: [],
          surfaces: [],
        },
      };
    },
    async cancel(_caller, command) {
      cancellations.push(command);
      return { ok: true, value: { kind: "receipt" } };
    },
    async *subscribe(_caller, _sessionId, _after, budget) {
      await new Promise((resolve) =>
        budget.signal.addEventListener("abort", resolve, { once: true }),
      );
    },
    async close() {
      return { ok: true, value: undefined };
    },
  };
  const service = createAccessService({
    host,
    sessionOptions: {
      provider: activeSession.binding.provider,
      accountRef: "account",
      config: { id: "cfg", revision: "1" },
      profile: "conversation",
    },
    now: () => 0,
    timeoutMs: 1_000,
  });
  t.after(() => service.close());
  const [server, transport] = localTransportPair();
  service.connect(server, fixtureCaller);
  const connection = client().connect(transport);
  t.after(() => connection.close());
  await connection.agent.request("initialize", {
    protocolVersion: 1,
    clientCapabilities: {},
  });
  await connection.agent.notify("session/cancel", {
    sessionId: activeSession.namespace.sessionId,
  });
  for (let i = 0; i < 100 && cancellations.length === 0; i++)
    await new Promise((resolve) => setTimeout(resolve, 5));
  return cancellations;
}

test("ACP cancel sends one request per Codex turn and preserves unsubmitted prompts", async (t) => {
  const records = [
    prompt("accepted", "queue_next", "accepted"),
    prompt(
      "start",
      "queue_next",
      "running",
      dispatch("a-start", "request-start"),
    ),
    prompt("steer", "steer", "running", dispatch("a-steer", "request-steer")),
    prompt(
      "foreign-thread",
      "queue_next",
      "running",
      dispatch("a-thread", "request-thread", { nativeThreadId: "thread-2" }),
    ),
    prompt(
      "foreign-generation",
      "queue_next",
      "running",
      dispatch("a-generation", "request-generation", {
        observerGeneration: "generation-1",
      }),
    ),
    prompt(
      "other-run",
      "queue_next",
      "running",
      dispatch("a-run", "request-run", { nativeRunId: "run-2" }),
    ),
    prompt(
      "terminal",
      "queue_next",
      "terminal",
      dispatch("a-terminal", "request-terminal"),
    ),
    prompt("invalidated", "queue_next", "invalidated"),
  ];
  const cancellations = await cancel(t, session, records);

  assert.deepEqual(
    cancellations.map((command) => command.input.targetCommandId).sort(),
    ["accepted", "start"],
  );
  assert.ok(
    cancellations.every(
      (command) =>
        command.input.generation === binding.generation &&
        command.input.nativeRunId === binding.nativeRunId,
    ),
  );
});

test("ACP cancel preserves providers without a native run identifier", async (t) => {
  const noRun = { ...binding, provider: "claude" };
  delete noRun.nativeRunId;
  delete noRun.nativeThreadId;
  const activeSession = { ...session, binding: noRun };
  const attempt = dispatch("a-no-run", "request-no-run", {
    nativeRunId: undefined,
    nativeThreadId: undefined,
  });
  const cancellations = await cancel(t, activeSession, [
    prompt("no-run", "queue_next", "running", attempt),
  ]);
  assert.equal(cancellations.length, 1);
  assert.equal(cancellations[0].input.targetCommandId, "no-run");
  assert.equal(cancellations[0].input.nativeRunId, undefined);
});
