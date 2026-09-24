import assert from "node:assert/strict";
import test from "node:test";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
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

    workingDirectory: cwd,
    permissions: "tools_disabled",
  };
  const resolved = {
    configuration,
    nativeDirectory: join(root, "native"),
    authentication: {
      type: "api_key",
      apiUrl: "http://127.0.0.1:9/v1",
      apiKey: "fixture-only",
    },
    model: "fixture",
  };
  const calls = [],
    replies = [],
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
      reply: (id, result) => replies.push({ id, result }),
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
                  type: "sessionFlags",
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
            model: resolved.model ?? "fixture",
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
    replies,
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

test("existing configuration is passed to Codex without login/account/token RPC", async (t) => {
  const f = await setup(t, {}, false);
  f.resolved.authentication = {
    type: "existing_config",
    directory: f.resolved.nativeDirectory,
  };
  f.resolved.verification = true;
  unwrap(
    await VerifiedProviderSession.open(f.adapter, f.configuration, budget()),
  );
  assert.equal(
    f.calls.some((c) => c.method.startsWith("account/")),
    false,
  );
  assert.equal(
    f.calls.find((c) => c.method === "thread/start").params.modelProvider,
    undefined,
  );
  assert.equal(
    f.calls.find((c) => c.method === "thread/start").params.ephemeral,
    true,
  );
});

// Each completed dispatch retains submitted + running + terminal observations.
async function fillObservationQueue(s, count = 341) {
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
    const result = await s.adapter.dispatch(
      s.admitted.binding,
      fixtureCommand(`fill-${current}`),
      s.attempt(`fill-${current}`),
      budget(),
    );
    assert.equal(result.certainty, "submitted");
  }
}

const reconciliationRecord = (s, command, attempt) => ({
  schemaVersion: 5,
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
test("stream output and terminal belong to start; each steer has its own acknowledgement", async (t) => {
  const s = await setup(t),
    binding = s.admitted.binding;
  const first = await s.adapter.dispatch(
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
  const redirected = await s.adapter.dispatch(
    first.binding,
    steer,
    s.attempt("steer", { nativeRunId: "turn-1" }),
    budget(),
  );
  assert.equal(redirected.certainty, "acknowledged");
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
    if (observed.filter((v) => v.body?.type === "terminal").length === 1) break;
  }
  assert.deepEqual(
    observed
      .filter((v) => v.body?.type === "terminal")
      .map((v) => [v.commandId, v.attemptId]),
    [["command-1", "attempt-command-1"]],
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
  const result = await s.adapter.dispatch(
    s.admitted.binding,
    command,
    attempt,
    budget(),
  );
  assert.deepEqual(result, {
    certainty: "unknown",
    correlationId: attempt.attemptId,
  });
  await s.adapter.dispatch(
    s.admitted.binding,
    fixtureCommand("second"),
    s.attempt("second"),
    budget(),
  );
  assert.equal(s.calls.filter((v) => v.method === "turn/start").length, 1);
  const record = {
    schemaVersion: 5,
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
  await s.adapter.dispatch(s.admitted.binding, command, attempt, budget());
  const record = {
    schemaVersion: 5,
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
  const child = createTestAdapter(s.options, s.runtime).ports();
  const result = await s.admitted.fork(
    child,
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
  assert.equal(result.source.binding.nativeThreadId, "thread-1");
  await child.agent.close(budget());
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
                  type: "sessionFlags",
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
  const started = await s.adapter.dispatch(
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
  const steered = await s.adapter.dispatch(
    started.binding,
    command,
    s.attempt("steer", { nativeRunId: "turn-1" }),
    budget(),
  );
  assert.equal(steered.certainty, "acknowledged");
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
    ["command-1"],
  );
  assert.equal(
    observations.filter(
      (v) => v.type === "acknowledged" && v.commandId === "steer",
    ).length,
    1,
  );
  const next = await s.adapter.dispatch(
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
  const result = await s.adapter.dispatch(
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
      await s.adapter.dispatch(
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
  await fillObservationQueue(s, 340);
  s.fault(undefined);
  const started = await s.adapter.dispatch(
    s.admitted.binding,
    fixtureCommand("active"),
    s.attempt("active"),
    budget(),
  );
  // 1020 completed observations + submitted/running + two deltas fill 1024 slots.
  for (const itemId of ["fills-slot-1023", "fills-slot-1024"])
    s.emit("item/agentMessage/delta", {
      threadId: "thread-1",
      turnId: started.binding.nativeRunId,
      itemId,
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
  const result = await s.adapter.dispatch(
    started.binding,
    command,
    s.attempt("overflow-steer", {
      nativeRunId: started.binding.nativeRunId,
    }),
    budget(),
  );
  assert.equal(result.certainty, "acknowledged");
  assert.equal(
    s.calls.some((call) => call.method === "runtime/close"),
    true,
  );
  assert.equal(
    (
      await s.adapter.dispatch(
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
    (await s.adapter.dispatch(s.admitted.binding, command, attempt, budget()))
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

// A requeue regression blocks the JS event loop, so the parent owns the time limit.
test("unmatched buffered notifications do not loop during another turn ACK", async (t) => {
  if (process.env.RSS_CODEX_REPLAY_CHILD !== "1") {
    const child = spawnSync(
      process.execPath,
      [
        "--test",
        "--test-name-pattern=^unmatched buffered notifications",
        fileURLToPath(import.meta.url),
      ],
      {
        env: { ...process.env, RSS_CODEX_REPLAY_CHILD: "1" },
        encoding: "utf8",
        timeout: 5000,
        maxBuffer: 256 * 1024,
      },
    );
    assert.equal(
      child.status,
      0,
      child.error?.message ?? child.stdout + child.stderr,
    );
    return;
  }
  const s = await setup(t, { nativeDiagnostics: false });
  s.emit("item/agentMessage/delta", {
    threadId: "thread-1",
    turnId: "unmatched",
    itemId: "old-item",
    delta: "unattributed",
  });
  const submitted = await s.adapter.dispatch(
    s.admitted.binding,
    fixtureCommand(),
    s.attempt("command-1"),
    budget(),
  );
  assert.equal(submitted.certainty, "submitted");
  s.thread.turns[0].status = "completed";
  s.emit("turn/completed", { threadId: "thread-1", turn: s.thread.turns[0] });
  const observed = [];
  for await (const item of s.adapter.observe(s.admitted.binding, {
    ...budget(),
    timeoutMs: 20,
  }))
    observed.push(item);
  assert.equal(
    observed.some((v) => v.body?.type === "terminal"),
    true,
  );
  assert.equal(
    observed.some((v) => v.type === "delta"),
    false,
  );
});

test("typed item/started correlates and replays earlier deltas after a lost response", async (t) => {
  const s = await setup(t);
  const completed = turn("early-turn", "attempt-command-1");
  s.fault((method) => {
    if (method !== "turn/start") return;
    s.emit("item/agentMessage/delta", {
      threadId: "thread-1",
      turnId: completed.id,
      itemId: "early-message",
      delta: "early",
    });
    s.emit("item/started", {
      threadId: "thread-1",
      turnId: completed.id,
      item: completed.items[0],
      startedAtMs: 1,
    });
    throw new Error("lost response");
  });
  const result = await s.adapter.dispatch(
    s.admitted.binding,
    fixtureCommand(),
    s.attempt("command-1"),
    budget(),
  );
  assert.equal(result.certainty, "submitted");
  assert.equal(result.binding.nativeRunId, completed.id);
  s.emit("turn/completed", { threadId: "thread-1", turn: completed });
  const observations = [];
  for await (const value of s.adapter.observe(s.admitted.binding, budget())) {
    observations.push(value);
    if (value.body?.type === "terminal") break;
  }
  const delta = observations.find((value) => value.type === "delta");
  assert.equal(delta.text, "early");
  assert.equal(delta.commandId, "command-1");
  assert.equal(delta.binding.nativeRunId, completed.id);
});

test("typed notifications retain runtime rejection of malformed payloads", async (t) => {
  for (const [method, payload] of [
    ["item/agentMessage/delta", { itemId: "bad", delta: 42 }],
    ["item/completed", { item: null }],
    ["turn/completed", { turn: { id: "turn-1", status: "bogus", items: [] } }],
  ]) {
    const s = await setup(t);
    const result = await s.adapter.dispatch(
      s.admitted.binding,
      fixtureCommand(),
      s.attempt("command-1"),
      budget(),
    );
    assert.equal(result.certainty, "submitted");
    s.emit(method, { threadId: "thread-1", turnId: "turn-1", ...payload });
    assert.ok(
      s.calls.some((call) => call.method === "runtime/close"),
      method,
    );
  }
});

test("unknown notification methods cannot confirm a lost submission", async (t) => {
  const s = await setup(t);
  s.fault((method, params) => {
    if (method !== "turn/start") return;
    s.emit("future/notification", {
      threadId: "thread-1",
      turnId: "foreign-turn",
      item: {
        type: "userMessage",
        id: "foreign-item",
        clientId: params.clientUserMessageId,
        content: [],
      },
    });
    throw new Error("lost response");
  });
  const result = await s.adapter.dispatch(
    s.admitted.binding,
    fixtureCommand(),
    s.attempt("command-1"),
    budget(),
  );
  assert.equal(result.certainty, "unknown");
  const observations = [];
  for await (const value of s.adapter.observe(s.admitted.binding, {
    ...budget(),
    timeoutMs: 20,
  }))
    observations.push(value);
  assert.equal(observations.length, 0);
});

test("diagnostics redact all native payloads and unknown method strings", async (t) => {
  const s = await setup(t);
  s.emit("PRIVATE_METHOD_CANARY", {
    text: "PRIVATE_BODY_CANARY",
    path: "/secret/path",
    arguments: { token: "PRIVATE_KEY_CANARY" },
  });
  const stream = s.adapter
    .diagnostics(s.admitted.binding, budget())
    [Symbol.asyncIterator]();
  const record = (await stream.next()).value;
  assert.equal(JSON.stringify(record).includes("PRIVATE_"), false);
  assert.equal("message" in record, false);
  assert.equal(record.kind, "other");
  await stream.return();
  for (const method of ["constructor", "__proto__", "toString"])
    s.emit(method, {});
  const next = s.adapter
    .diagnostics(s.admitted.binding, budget())
    [Symbol.asyncIterator]();
  for (let i = 0; i < 3; i++)
    assert.equal((await next.next()).value.kind, "other");
  await next.return();
});

test("absent and slow diagnostic consumers never terminate a healthy incarnation", async (t) => {
  const s = await setup(t);
  for (let i = 0; i < 1025; i++)
    s.emit("diagnostic-only", { text: "sensitive" });
  assert.equal(
    s.calls.some((c) => c.method === "runtime/close"),
    false,
  );
  const stream = s.adapter
    .diagnostics(s.admitted.binding, budget())
    [Symbol.asyncIterator]();
  assert.ok((await stream.next()).value.dropped > 0);
  for (let i = 0; i < 1025; i++) s.emit("diagnostic-only", {});
  await stream.return();
  const result = await s.adapter.dispatch(
    s.admitted.binding,
    fixtureCommand(),
    s.attempt("command-1"),
    budget(),
  );
  assert.equal(result.certainty, "submitted");
});

test("Host owns fork admission, child cleanup and the three explicit ports", async (t) => {
  for (const scenario of [
    "foreign-tenant",
    "missing-terminal",
    "lost-receipt",
    "rejected-verifier",
  ]) {
    const s = await setup(t);
    s.thread.turns.push(turn("terminal-turn", "old-client"));
    const child = createTestAdapter(s.options, s.runtime).ports();
    assert.equal("fork" in child.agent, false);
    assert.equal("readHistory" in child.agent, false);
    assert.equal("diagnostics" in child.agent, false);
    assert.deepEqual(Object.keys(child.diagnostics), ["observe"]);
    const configuration = {
      ...s.configuration,
      namespace: { ...s.configuration.namespace, sessionId: "child" },
    };
    if (scenario === "foreign-tenant")
      configuration.namespace.tenantId = "foreign";
    if (scenario === "rejected-verifier")
      configuration.permissions = "host_mediated";
    if (scenario === "lost-receipt")
      s.fault((method) => {
        if (method === "thread/fork") throw new Error("transport lost");
      });
    const result = await s.admitted.fork(
      child,
      scenario === "missing-terminal" ? "missing" : "terminal-turn",
      configuration,
      budget(),
    );
    assert.equal(
      result.certainty,
      scenario === "lost-receipt" ? "unknown" : "not_created",
    );
    assert.equal(result.cleanupError, undefined);
    assert.equal(
      s.calls.filter((c) => c.method === "thread/fork").length,
      scenario === "lost-receipt" ? 1 : 0,
    );
    // A retry cannot create a second native child, even after an uncertain outcome.
    const retry = await s.admitted.fork(
      child,
      "terminal-turn",
      configuration,
      budget(),
    );
    assert.equal(retry.certainty, "not_created");
  }
});

test("explicit models cannot silently fall back and actual returned identity is required", async (t) => {
  const f = await setup(t, {}, false);
  f.fault((method) =>
    method === "thread/start"
      ? { thread: f.thread, model: "another-model" }
      : undefined,
  );
  const result = await VerifiedProviderSession.open(
    f.adapter,
    f.configuration,
    budget(),
  );
  assert.equal(result.ok, false);
  assert.equal(result.error.code, "unsupported_capability");
  assert.equal(
    f.calls.find((c) => c.method === "thread/start").params
      .allowProviderModelFallback,
    false,
  );
});

const failures = [
  ["unauthorized", "authentication_required"],
  ["badRequest", "invalid_input"],
  ["usageLimitExceeded", "limit_exceeded"],
  ["rateLimitExceeded", "limit_exceeded"],
  ["sessionBudgetExceeded", "limit_exceeded"],
  ["other", "unavailable"],
  ["internalServerError", "unavailable"],
  ...[
    "httpConnectionFailed",
    "responseStreamConnectionFailed",
    "responseStreamDisconnected",
    "responseTooManyFailedAttempts",
  ].flatMap((tag) =>
    [
      [401, "authentication_required"],
      [400, "invalid_input"],
      [403, "permission_denied"],
      [429, "limit_exceeded"],
      [500, "unavailable"],
      [null, "unavailable"],
    ].map(([httpStatusCode, code]) => [{ [tag]: { httpStatusCode } }, code]),
  ),
];
for (const path of ["live", "before_ack"]) {
  for (const [info, expected] of failures) {
    test(`closed Codex failure ${path} ${JSON.stringify(info)}`, async (t) => {
      const s = await setup(t),
        command = fixtureCommand("failure"),
        attempt = s.attempt("failure");
      const error = {
        message: "CANARY_RAW_AUTH",
        codexErrorInfo: info,
        additionalDetails: null,
      };
      if (path === "before_ack")
        s.fault(async (method, params) => {
          if (method !== "turn/start") return;
          const failed = {
            ...turn("failed-turn", params.clientUserMessageId, "failed"),
            error,
          };
          s.thread.turns.push(failed);
          s.emit("turn/completed", { threadId: s.thread.id, turn: failed });
          await new Promise(setImmediate);
          return { turn: failed };
        });
      const submitted = await s.adapter.dispatch(
        s.admitted.binding,
        command,
        attempt,
        budget(),
      );
      if (path === "live") {
        assert.equal(submitted.certainty, "submitted");
        Object.assign(s.thread.turns[0], { status: "failed", error });
        s.emit("turn/completed", {
          threadId: s.thread.id,
          turn: s.thread.turns[0],
        });
        s.emit("turn/completed", {
          threadId: s.thread.id,
          turn: s.thread.turns[0],
        });
      } else assert.equal(submitted.certainty, "submitted");
      const events = [];
      for await (const event of s.adapter.observe(
        s.admitted.binding,
        budget(),
      )) {
        events.push(event);
        if (event.body?.type === "terminal") break;
      }
      assert.deepEqual(
        events
          .filter((e) => e.body?.type === "error")
          .map((e) => e.body.failure),
        [{ code: expected, retry: "never" }],
      );
      assert.equal(events.at(-1).body.outcome, "failed");
      assert.equal(JSON.stringify(events).includes("CANARY"), false);
    });
  }
}
