import assert from "node:assert/strict";
import test from "node:test";
import { mkdtemp, mkdir, rm, realpath } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createTestAdapter } from "../../../packages/ai-adapters/codex/dist/testing.js";
import { nativeSettings } from "../../../packages/ai-adapters/codex/dist/configuration.js";
import { VerifiedProviderSession } from "../../../packages/ai-contract/dist/session.js";
import {
  runProviderConformance,
  fixtureCaller,
  fixtureCommand,
  unwrap,
} from "../../../packages/ai-contract/dist/testing/index.js";

const budget = () => ({
  timeoutMs: 2000,
  signal: new AbortController().signal,
});
const turn = (id, clientId, status = "completed") => ({
  id,
  status,
  itemsView: "full",
  error: null,
  items: [
    {
      type: "userMessage",
      id: `user-${id}-${clientId}`,
      clientId,
      content: [],
    },
  ],
  startedAt: 1,
  completedAt: 2,
  durationMs: 1,
});
async function setup(t, overrides = {}, admit = true) {
  const root = await realpath(
    await mkdtemp(join(tmpdir(), "rss-codex-fixture-")),
  );
  const cwd = join(root, "cwd");
  await mkdir(cwd);
  const configuration = {
    namespace: { ...fixtureCaller, sessionId: "session-1" },
    provider: "codex",
    config: { id: "cfg", revision: "1" },
    accountRef: "account",
    workingDirectory: cwd,
    permissions: "tools_disabled",
  };
  const resolved = {
    configuration,
    nativeDirectory: join(root, "native"),
    apiUrl: "http://127.0.0.1:9/v1",
    apiKey: "fixture-only",
    model: "fixture",
  };
  const calls = [],
    rejected = [],
    ports = [];
  const thread = {
    id: "thread-1",
    sessionId: "native-1",
    forkedFromId: null,
    parentThreadId: null,
    cwd,
    cliVersion: "0.155.0",
    environments: [],
    turns: [],
  };
  let listener, stopped, fault;
  const runtime = (spec) => {
    const port = {
      spec,
      stopped: new Promise((r) => {
        stopped = r;
      }),
      listen: (f) => {
        listener = f;
      },
      notify: (method) => calls.push({ method }),
      reject: (id) => rejected.push(id),
      reply: () => assert.fail("unsolicited request allowed"),
      close: async () => {
        calls.push({ method: "runtime/close" });
        stopped();
        return { ok: true, value: { processStopped: true } };
      },
      request: async (method, params) => {
        calls.push({ method, params });
        if (fault) {
          const value = await fault(method, params);
          if (value !== undefined) return value;
        }
        if (method === "initialize")
          return {
            userAgent: "fixture/0.155.0",
            codexHome: resolved.nativeDirectory,
          };
        if (method === "config/read")
          return {
            config: nativeSettings(resolved),
            layers: [
              {
                name: {
                  type: "user",
                  file: join(resolved.nativeDirectory, "config.toml"),
                  profile: null,
                },
                config: nativeSettings(resolved),
              },
              {
                name: { type: "system", file: "/etc/codex/config.toml" },
                config: {},
              },
            ],
          };
        if (method === "mcpServerStatus/list")
          return { data: [], nextCursor: null };
        if (method === "thread/turns/list")
          return { data: thread.turns, nextCursor: null };
        if (method === "thread/read") return { thread };
        if (["thread/start", "thread/resume", "thread/fork"].includes(method))
          return {
            thread:
              method === "thread/fork"
                ? {
                    ...thread,
                    id: "fork-thread",
                    sessionId: "fork-session",
                    forkedFromId: thread.id,
                  }
                : thread,
            approvalPolicy: "on-request",
            sandbox: { type: "readOnly", networkAccess: false },
            environments: [],
            instructionSources: [],
          };
        if (method === "turn/start") {
          const value = turn(
            `turn-${thread.turns.length + 1}`,
            params.clientUserMessageId,
            "inProgress",
          );
          thread.turns.push(value);
          return { turn: value };
        }
        if (method === "turn/steer") {
          thread.turns.at(-1).items.push({
            type: "userMessage",
            id: "steer-item",
            clientId: params.clientUserMessageId,
            content: [],
          });
          return { turnId: params.expectedTurnId };
        }
        if (method === "turn/interrupt") return {};
        throw new Error(`unexpected ${method}`);
      },
    };
    ports.push(port);
    return port;
  };
  const options = {
    clock: { now: () => 0 },
    nativeDiagnostics: true,
    resolveConfiguration: async (identity) => ({
      ...resolved,
      configuration: { ...configuration, namespace: identity.namespace },
      ...(identity.history
        ? {
            ownedHistory: {
              nativeSessionId: identity.history.nativeSessionId,
              nativeThreadId: identity.history.nativeThreadId,
            },
          }
        : {}),
    }),
    ...overrides,
  };
  const adapter = createTestAdapter(options, runtime);
  t.after(async () => {
    await adapter.close(budget());
    for (const port of ports) await port.close(budget());
    await rm(root, { recursive: true, force: true });
  });
  const admitted = admit
    ? unwrap(
        await VerifiedProviderSession.open(adapter, configuration, budget()),
      )
    : undefined;
  const attempt = (id, extra = {}) => ({
    attemptId: `attempt-${id}`,
    originGeneration: admitted.binding.generation,
    observerGeneration: admitted.binding.generation,
    nativeSessionId: admitted.binding.nativeSessionId,
    nativeThreadId: admitted.binding.nativeThreadId,
    certainty: "intent",
    ...extra,
  });
  return {
    adapter,
    admitted,
    configuration,
    calls,
    rejected,
    thread,
    resolved,
    options,
    runtime,
    attempt,
    emit: (method, params, id) =>
      listener({ method, params, ...(id === undefined ? {} : { id }) }),
    fault: (f) => {
      fault = f;
    },
  };
}

async function fillObservationQueue(s, count = 511) {
  let index = 0;
  s.fault((method, params) => {
    if (method !== "turn/start" || index >= count) return;
    const value = turn(
      `fill-turn-${index++}`,
      params.clientUserMessageId,
      "completed",
    );
    return { turn: value };
  });
  for (let current = 0; current < count; current++) {
    const result = await s.adapter.submit(
      s.admitted.binding,
      fixtureCommand(`fill-${current}`),
      s.attempt(`fill-${current}`),
      budget(),
    );
    assert.equal(result.certainty, "submitted");
  }
}

const reconciliationRecord = (s, command, attempt) => ({
  schemaVersion: 2,
  kind: "commandRecord",
  command,
  receipt: { namespace: s.configuration.namespace },
  state: "reconciliation_required",
  dispatch: {
    ...attempt,
    certainty: "unknown",
    correlationId: attempt.attemptId,
  },
});

test("pinned admission seals native capability negotiation and separates thread/session IDs", async (t) => {
  const s = await setup(t);
  assert.equal(s.admitted.binding.nativeSessionId, "native-1");
  assert.equal(s.admitted.binding.nativeThreadId, "thread-1");
  assert.equal(s.admitted.capabilities.steer, "supported");
  assert.equal(s.admitted.capabilities.fork, "supported");
  assert.equal(s.admitted.capabilities.subagent, "unsupported");
  const params = s.calls.find((c) => c.method === "thread/start").params;
  assert.deepEqual(params.dynamicTools, []);
  assert.deepEqual(params.environments, []);
  assert.equal(params.approvalPolicy, "on-request");
  assert.equal(params.sandbox, "read-only");
});
test("stream output belongs to start; start and each steer terminate with their own attempt", async (t) => {
  const s = await setup(t),
    binding = s.admitted.binding;
  const first = await s.adapter.submit(
    binding,
    fixtureCommand(),
    s.attempt("command-1"),
    budget(),
  );
  assert.equal(first.certainty, "submitted");
  const steer = {
    ...fixtureCommand("steer"),
    input: {
      type: "prompt",
      policy: "steer",
      targetRunId: "turn-1",
      text: "change",
    },
  };
  const redirected = await s.adapter.submit(
    first.binding,
    steer,
    s.attempt("steer", { nativeRunId: "turn-1" }),
    budget(),
  );
  assert.equal(redirected.certainty, "submitted");
  assert.notEqual(
    first.binding.nativeRequestId,
    redirected.binding.nativeRequestId,
  );
  s.emit("item/agentMessage/delta", {
    threadId: "thread-1",
    turnId: "turn-1",
    itemId: "message-1",
    delta: "hello",
  });
  s.emit("item/completed", {
    threadId: "thread-1",
    turnId: "turn-1",
    item: { type: "agentMessage", id: "message-1", text: "hello" },
  });
  s.thread.turns[0].status = "completed";
  s.emit("turn/completed", { threadId: "thread-1", turn: s.thread.turns[0] });
  const observed = [];
  for await (const item of s.adapter.observe(binding, budget())) {
    observed.push(item);
    if (observed.filter((v) => v.body?.type === "terminal").length === 2) break;
  }
  assert.deepEqual(
    observed
      .filter((v) => v.body?.type === "terminal")
      .map((v) => [v.commandId, v.attemptId]),
    [
      ["command-1", "attempt-command-1"],
      ["steer", "attempt-steer"],
    ],
  );
  assert.equal(observed.find((v) => v.type === "delta").commandId, "command-1");
  assert.equal(
    observed.find((v) => v.body?.type === "text").body.messageId,
    "message-1",
  );
});
test("lost submit response is reconciled by clientId; no blind second start", async (t) => {
  const s = await setup(t),
    command = fixtureCommand(),
    attempt = s.attempt("command-1");
  s.fault((method, params) => {
    if (method === "turn/start") {
      s.thread.turns.push(turn("turn-lost", params.clientUserMessageId));
      throw new Error("connection lost");
    }
  });
  const result = await s.adapter.submit(
    s.admitted.binding,
    command,
    attempt,
    budget(),
  );
  assert.deepEqual(result, {
    certainty: "unknown",
    correlationId: attempt.attemptId,
  });
  await s.adapter.submit(
    s.admitted.binding,
    fixtureCommand("second"),
    s.attempt("second"),
    budget(),
  );
  assert.equal(s.calls.filter((v) => v.method === "turn/start").length, 1);
  const record = {
    schemaVersion: 2,
    kind: "commandRecord",
    command,
    receipt: { namespace: s.configuration.namespace },
    state: "reconciliation_required",
    dispatch: {
      ...attempt,
      certainty: "unknown",
      correlationId: attempt.attemptId,
    },
  };
  const reconciled = unwrap(
    await s.adapter.reconcile(s.admitted.binding, record, budget()),
  );
  assert.equal(reconciled.status, "terminal");
  assert.equal(reconciled.binding.nativeRunId, "turn-lost");
  assert.equal(reconciled.binding.nativeRequestId, attempt.attemptId);
});
test("missing native history is unknown and reverse dynamic/approval calls cannot reach host", async (t) => {
  const s = await setup(t),
    command = fixtureCommand(),
    attempt = s.attempt("command-1");
  s.fault((method) => {
    if (method === "turn/start") throw new Error("unknown");
  });
  await s.adapter.submit(s.admitted.binding, command, attempt, budget());
  const record = {
    schemaVersion: 2,
    kind: "commandRecord",
    command,
    receipt: { namespace: s.configuration.namespace },
    state: "reconciliation_required",
    dispatch: {
      ...attempt,
      certainty: "unknown",
      correlationId: attempt.attemptId,
    },
  };
  assert.equal(
    unwrap(await s.adapter.reconcile(s.admitted.binding, record, budget()))
      .status,
    "unknown",
  );
  for (const [index, method] of [
    "item/tool/call",
    "item/commandExecution/requestApproval",
    "item/fileChange/requestApproval",
    "mcpServer/elicitation/request",
  ].entries())
    s.emit(method, {}, index);
  assert.deepEqual(s.rejected, [0, 1, 2, 3]);
});
test("fork preserves actual native identities and verifies the terminal source turn", async (t) => {
  const s = await setup(t);
  s.thread.turns.push(turn("terminal-turn", "old-client"));
  const result = await s.adapter.fork(
    s.admitted.binding,
    "terminal-turn",
    {
      ...s.configuration,
      namespace: { ...s.configuration.namespace, sessionId: "fork-logical" },
    },
    budget(),
  );
  assert.equal(result.certainty, "created");
  assert.equal(result.session.binding.nativeSessionId, "fork-session");
  assert.equal(result.session.binding.nativeThreadId, "fork-thread");
  assert.equal(result.source.nativeThreadId, "thread-1");
  await result.port.close(budget());
  assert.equal(s.calls.filter((c) => c.method === "turn/start").length, 0);
});

// The shared harness uses the real adapter with a bounded, finite scripted observer.
test("Codex participates in shared ProviderAgentPort conformance", async (t) => {
  const fixture = await setup(t, {}, false);
  await runProviderConformance(
    async (scenario) => {
      const s = await setup(
        t,
        {
          resolveConfiguration: async () => ({
            ...fixture.resolved,
            configuration: fixture.configuration,
          }),
        },
        false,
      );
      // Each peer must acknowledge the same explicit workspace and private CODEX_HOME.
      Object.assign(s.thread, { cwd: fixture.configuration.workingDirectory });
      s.fault((method) => {
        if (method === "initialize")
          return {
            userAgent: "fixture/0.155.0",
            codexHome: fixture.resolved.nativeDirectory,
          };
        if (method === "config/read")
          return {
            config: nativeSettings(fixture.resolved),
            layers: [
              {
                name: {
                  type: "user",
                  file: join(fixture.resolved.nativeDirectory, "config.toml"),
                  profile: null,
                },
                config: nativeSettings(fixture.resolved),
              },
            ],
          };
        if (scenario === "unknown" && method === "turn/start")
          throw new Error("unknown dispatch");
      });
      const observe = s.adapter.observe.bind(s.adapter);
      s.adapter.observe = (binding, b) =>
        observe(binding, { ...b, timeoutMs: 20 });
      return s.adapter;
    },
    fixture.configuration,
    budget,
  );
});

test("terminal arriving before steer ACK closes the acknowledged steer exactly once", async (t) => {
  const s = await setup(t);
  const started = await s.adapter.submit(
    s.admitted.binding,
    fixtureCommand(),
    s.attempt("command-1"),
    budget(),
  );
  s.fault((method) => {
    if (method !== "turn/steer") return;
    s.thread.turns[0].status = "completed";
    s.emit("turn/completed", { threadId: "thread-1", turn: s.thread.turns[0] });
    return { turnId: "turn-1" };
  });
  const command = {
    ...fixtureCommand("steer"),
    input: {
      type: "prompt",
      policy: "steer",
      targetRunId: "turn-1",
      text: "change",
    },
  };
  const steered = await s.adapter.submit(
    started.binding,
    command,
    s.attempt("steer", { nativeRunId: "turn-1" }),
    budget(),
  );
  assert.equal(steered.certainty, "submitted");
  const observations = [];
  for await (const item of s.adapter.observe(s.admitted.binding, {
    ...budget(),
    timeoutMs: 20,
  }))
    observations.push(item);
  assert.deepEqual(
    observations
      .filter((v) => v.body?.type === "terminal")
      .map((v) => v.commandId),
    ["command-1", "steer"],
  );
  assert.equal(
    observations.filter(
      (v) => v.type === "submitted" && v.commandId === "steer",
    ).length,
    1,
  );
  const next = await s.adapter.submit(
    s.admitted.binding,
    fixtureCommand("next"),
    s.attempt("next"),
    budget(),
  );
  assert.equal(next.certainty, "submitted");
});

test("ACK replay overflow fails the incarnation without fabricating a terminal", async (t) => {
  const s = await setup(t, { nativeDiagnostics: false });
  await fillObservationQueue(s);
  s.fault((method, params) => {
    if (method !== "turn/start") return;
    const active = turn(
      "overflow-turn",
      params.clientUserMessageId,
      "inProgress",
    );
    s.thread.turns.push(active);
    for (const itemId of ["overflow-delta-1", "overflow-delta-2"])
      s.emit("item/agentMessage/delta", {
        threadId: "thread-1",
        turnId: active.id,
        itemId,
        delta: itemId,
      });
    s.emit("turn/completed", {
      threadId: "thread-1",
      turn: { ...active, status: "completed" },
    });
    return { turn: active };
  });
  const result = await s.adapter.submit(
    s.admitted.binding,
    fixtureCommand("overflow"),
    s.attempt("overflow"),
    budget(),
  );
  assert.equal(result.certainty, "submitted");
  assert.equal(
    s.calls.some((call) => call.method === "runtime/close"),
    true,
  );
  const observations = [];
  for await (const item of s.adapter.observe(s.admitted.binding, {
    ...budget(),
    timeoutMs: 500,
  }))
    observations.push(item);
  assert.equal(
    observations.some(
      (item) => item.commandId === "overflow" && item.body?.type === "terminal",
    ),
    false,
  );
  assert.equal(
    (
      await s.adapter.submit(
        s.admitted.binding,
        fixtureCommand("after-overflow"),
        s.attempt("after-overflow"),
        budget(),
      )
    ).certainty,
    "not_sent",
  );
});

test("steer ACK overflow preserves submission fact and fails the incarnation", async (t) => {
  const s = await setup(t, { nativeDiagnostics: false });
  await fillObservationQueue(s);
  s.fault(undefined);
  const started = await s.adapter.submit(
    s.admitted.binding,
    fixtureCommand("active"),
    s.attempt("active"),
    budget(),
  );
  s.emit("item/agentMessage/delta", {
    threadId: "thread-1",
    turnId: started.binding.nativeRunId,
    itemId: "fills-last-slot",
    delta: "full",
  });
  const command = {
    ...fixtureCommand("overflow-steer"),
    input: {
      type: "prompt",
      policy: "steer",
      targetRunId: started.binding.nativeRunId,
      text: "change",
    },
  };
  const result = await s.adapter.submit(
    started.binding,
    command,
    s.attempt("overflow-steer", {
      nativeRunId: started.binding.nativeRunId,
    }),
    budget(),
  );
  assert.equal(result.certainty, "submitted");
  assert.equal(
    s.calls.some((call) => call.method === "runtime/close"),
    true,
  );
  assert.equal(
    (
      await s.adapter.submit(
        s.admitted.binding,
        fixtureCommand("after-steer-overflow"),
        s.attempt("after-steer-overflow"),
        budget(),
      )
    ).certainty,
    "not_sent",
  );
});

test("reconcile replay overflow fails closed and stops the incarnation", async (t) => {
  const s = await setup(t, { nativeDiagnostics: false });
  await fillObservationQueue(s);
  const command = fixtureCommand("reconcile-overflow"),
    attempt = s.attempt("reconcile-overflow");
  s.fault((method, params) => {
    if (method !== "turn/start") return;
    const active = turn(
      "reconcile-overflow-turn",
      params.clientUserMessageId,
      "inProgress",
    );
    s.thread.turns.push(active);
    for (const itemId of ["replay-1", "replay-2", "replay-3"])
      s.emit("item/agentMessage/delta", {
        threadId: "thread-1",
        turnId: active.id,
        itemId,
        delta: itemId,
      });
    throw new Error("lost ACK");
  });
  assert.equal(
    (await s.adapter.submit(s.admitted.binding, command, attempt, budget()))
      .certainty,
    "unknown",
  );
  s.fault(undefined);
  assert.deepEqual(
    await s.adapter.reconcile(
      s.admitted.binding,
      reconciliationRecord(s, command, attempt),
      budget(),
    ),
    {
      ok: false,
      error: { code: "unavailable", retry: "reconcile_first" },
    },
  );
  assert.equal(
    s.calls.some((call) => call.method === "runtime/close"),
    true,
  );
});
