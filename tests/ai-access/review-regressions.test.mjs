import { replaceStage } from "../../packages/ai-contract/dist/index.js";
import { activeStage } from "../../packages/ai-contract/dist/index.js";
import assert from "node:assert/strict";
import { test } from "node:test";
import { createRequire } from "node:module";
import { createAccessService } from "../../packages/ai-access/dist/index.js";
import {
  RuntimeClient,
  localTransportPair,
  channelStream,
} from "../../packages/ai-client/dist/index.js";
import {
  FakeHost,
  fixtureCaller,
  unwrap,
} from "../../packages/ai-contract/dist/testing/index.js";
const require = createRequire(
  new URL("../../packages/ai-access/package.json", import.meta.url),
);
const { client } = await import(require.resolve("@agentclientprotocol/sdk"));
const options = {
  provider: "fake",

  config: { id: "cfg", revision: "1" },
  profile: "conversation",
};
const delay = (ms) => new Promise((r) => setTimeout(r, ms));
async function until(read) {
  for (let i = 0; i < 100; i++) {
    const result = await read();
    if (result) return result;
    await delay(5);
  }
  throw new Error("condition timed out");
}
function setup(t, extra = {}) {
  const host = new FakeHost();
  const service = createAccessService({
    host,
    sessionOptions: { connectionId: "cfg" },
    now: () => 0,
    timeoutMs: 1000,
    ...extra,
  });
  t.after(() => service.close());
  return { host, service };
}
async function standard(
  t,
  service,
  onPermission = async () => ({
    outcome: { outcome: "selected", optionId: "allow" },
  }),
) {
  const [a, b] = localTransportPair();
  service.connect(a, fixtureCaller);
  const connection = client()
    .onNotification("session/update", () => {})
    .onRequest("session/request_permission", ({ params, signal }) =>
      onPermission(params, signal),
    )
    .connect(b);
  t.after(() => connection.close());
  await connection.agent.request("initialize", {
    protocolVersion: 1,
    clientCapabilities: {},
  });
  return connection.agent;
}
const permission = (sessionId) => ({
  sessionId,
  toolCall: { toolCallId: "tool", title: "Question" },
  options: [{ optionId: "allow", name: "Allow", kind: "allow_once" }],
});
async function runtime(t, service) {
  const [a, b] = localTransportPair();
  service.connect(a, fixtureCaller);
  const r = new RuntimeClient(b);
  t.after(() => r.close());
  await r.initialize();
  return r;
}
test("long prompt and subscription survive the request timeout; late permission and terminal still arrive", async (t) => {
  const { host, service } = setup(t, { timeoutMs: 40 });
  const agent = await standard(t, service);
  const { sessionId } = await agent.request("session/new", {
    cwd: "/",
    mcpServers: [],
  });
  let result;
  const pending = agent
    .request("session/prompt", {
      sessionId,
      prompt: [{ type: "text", text: "slow" }],
    })
    .then(
      (value) => (result = { value }),
      (error) => (result = { error }),
    );
  const command = await until(
    async () =>
      unwrap(
        await host.store.snapshotPage(
          { ...fixtureCaller, sessionId },
          { limit: 64 },
        ),
      ).commands[0]?.command,
  );
  await delay(90);
  assert.equal(
    result,
    undefined,
    "admission timeout must not terminate a live prompt",
  );
  assert.equal(
    (
      await service.requestPermission(
        fixtureCaller,
        permission(sessionId),
        AbortSignal.timeout(500),
      )
    ).outcome.optionId,
    "allow",
  );
  unwrap(
    await host.advance(fixtureCaller, sessionId, command.commandId, [
      { type: "terminal", outcome: "completed" },
    ]),
  );
  await pending;
  assert.equal(result.value.stopReason, "end_turn");
});
test("session cancel aborts only its own pending permissions and rejects a late approval", async (t) => {
  const { service } = setup(t);
  const deliveries = new Map();
  const agent = await standard(
    t,
    service,
    async (request, signal) =>
      new Promise((resolve) => {
        deliveries.set(request.sessionId, { signal, resolve });
        signal.addEventListener(
          "abort",
          () =>
            resolve({ outcome: { outcome: "selected", optionId: "allow" } }),
          { once: true },
        );
      }),
  );
  const ids = [];
  for (let i = 0; i < 2; i++)
    ids.push(
      (await agent.request("session/new", { cwd: "/", mcpServers: [] }))
        .sessionId,
    );
  const pending = ids.map((id) =>
    service.requestPermission(
      fixtureCaller,
      permission(id),
      AbortSignal.timeout(1000),
    ),
  );
  await until(() => deliveries.size === 2);
  await agent.notify("session/cancel", { sessionId: ids[0] });
  await until(() => deliveries.get(ids[0]).signal.aborted);
  assert.equal((await pending[0]).outcome.outcome, "cancelled");
  assert.equal(deliveries.get(ids[1]).signal.aborted, false);
  deliveries
    .get(ids[1])
    .resolve({ outcome: { outcome: "selected", optionId: "allow" } });
  assert.equal((await pending[1]).outcome.optionId, "allow");
});
test("service close is terminal, aborts pumps immediately and awaits owned work without closing Host", async (t) => {
  const { host, service } = setup(t);
  const original = host.subscribe.bind(host);
  let signal,
    finished = false;
  host.subscribe = async function* (...args) {
    signal = args[3].signal;
    try {
      yield* original(...args);
    } finally {
      finished = true;
    }
  };
  const agent = await standard(t, service);
  await agent.request("session/new", { cwd: "/", mcpServers: [] });
  await until(() => signal);
  const closing = service.close();
  assert.equal(signal.aborted, true);
  assert.equal(typeof closing?.then, "function");
  await closing;
  assert.equal(finished, true);
  assert.throws(
    () => service.connect(localTransportPair()[0], fixtureCaller),
    /closed|unavailable/,
  );
  assert.equal(
    (
      await host.openSessionForTest(fixtureCaller, options, {
        signal: new AbortController().signal,
        timeoutMs: 1000,
      })
    ).ok,
    true,
  );
  assert.equal(await service.close(), undefined);
});
test("send rejection closes receive side and disposes the local listener exactly once", async () => {
  let receive,
    disposed = 0;
  const failure = new Error("send failed");
  const stream = channelStream({
    listen(fn) {
      receive = fn;
      return () => disposed++;
    },
    async send() {
      throw failure;
    },
  });
  const reader = stream.readable.getReader(),
    writer = stream.writable.getWriter();
  await assert.rejects(
    writer.write({ jsonrpc: "2.0", method: "hello" }),
    (e) => e.code === "transport_failed" && !("cause" in e),
  );
  assert.equal(disposed, 1);
  receive({ jsonrpc: "2.0", method: "late" });
  assert.equal((await reader.read()).done, true);
  await reader.cancel();
  assert.equal(disposed, 1);
});
test("unexpected subscription completion marks product view for resync and emits a value-free diagnostic", async (t) => {
  const codes = [];
  const { host, service } = setup(t, {
    onDiagnostic: (code) => codes.push(code),
  });
  host.subscribe = async function* () {};
  const r = await runtime(t, service);
  const view = await r.createSession();
  const id = view.namespace.sessionId;
  await until(() => r.getSession(id).connection === "resync_required");
  assert.deepEqual(codes, ["subscription_ended"]);
});
test("subscription exception is observable without disclosing its message", async (t) => {
  const codes = [];
  const { host, service } = setup(t, {
    onDiagnostic: (code) => codes.push(code),
  });
  host.subscribe = async function* () {
    throw new Error("secret-provider-payload");
  };
  const r = await runtime(t, service);
  const view = await r.createSession();
  const id = view.namespace.sessionId;
  await until(() => r.getSession(id).connection === "resync_required");
  assert.deepEqual(codes, ["subscription_failed"]);
});
test("Fake Host never advertises durable receipts from an in-memory implementation", () => {
  const host = new FakeHost();
  for (const durableReceipts of [false, true])
    assert.equal(
      unwrap(
        host.negotiate({
          contractVersion: 5,
          acp: 1,
          cursorAttach: true,
          durableReceipts,
        }),
      ).durableReceipts,
      false,
    );
});

test(
  "service enforces configured limits on the original transport envelope",
  { timeout: 2000 },
  async (t) => {
    const { service } = setup(t, {
      limits: {
        maxBytes: 256,
        maxTextBytes: 128,
        maxDepth: 32,
        maxNodes: 16384,
      },
    });
    const [a, b] = localTransportPair();
    const server = service.connect(a, fixtureCaller);
    const connection = client().connect(b);
    t.after(() => connection.close());
    await assert.rejects(
      connection.agent.request("initialize", {
        protocolVersion: 1,
        clientCapabilities: {},
        clientInfo: { name: "x".repeat(1024), version: "1" },
      }),
    );
    await server.closed;
    assert.equal(server.signal.aborted, true);
  },
);

test("resume detaches the old generation before Host changes it, then rebuilds from the new snapshot", async (t) => {
  const { host, service } = setup(t);
  host.createSession = (caller, _options, budget) =>
    host.openSessionForTest(caller, options, budget);
  let generation, oldSignal;
  const snapshot = host.snapshotPage.bind(host),
    subscribe = host.subscribe.bind(host);
  host.snapshotPage = async (...args) => {
    const result = await snapshot(...args);
    if (result.ok && generation)
      activeStage(result.value.session).binding.generation = generation;
    return result;
  };
  host.subscribe = async function* (...args) {
    oldSignal = args[3].signal;
    for await (const item of subscribe(...args)) {
      if (item.type === "event" && generation)
        item.event.generation = generation;
      yield item;
    }
  };
  host.resume = async (caller, id, budget) => {
    assert.equal(
      oldSignal.aborted,
      true,
      "old attachment must be gone before resume",
    );
    generation = "resumed-generation";
    return {
      ok: true,
      value: unwrap(await host.snapshotPage(caller, id, { limit: 1 }, budget))
        .session,
    };
  };
  const r = await runtime(t, service);
  const initial = await r.createSession(),
    id = initial.namespace.sessionId;
  const states = [];
  r.observe((view) => states.push(view.connection));
  const resumed = await r.resume(id);
  assert.equal(resumed.generation, "resumed-generation");
  assert.equal(resumed.connection, "attached");
  assert.equal("session" in resumed, false);
  assert.equal(states.includes("resync_required"), false);
  await r.submit({
    schemaVersion: 5,
    kind: "command",
    sessionId: id,
    commandId: "after-resume",
    expiresAtMs: 1000,
    input: { type: "prompt", policy: "queue_next", text: "new generation" },
  });
  await until(() => r.getSession(id).cursor === 1);
  assert.equal(r.getSession(id).connection, "attached");
});

test("RuntimeClient preserves tool state and content across live updates and snapshot recovery", async (t) => {
  const { host, service } = setup(t),
    r = await runtime(t, service);
  const id = (await r.createSession()).namespace.sessionId;
  await r.submit({
    schemaVersion: 5,
    kind: "command",
    sessionId: id,
    commandId: "tools",
    expiresAtMs: 1000,
    input: { type: "prompt", policy: "queue_next", text: "tools" },
  });
  unwrap(
    await host.advance(fixtureCaller, id, "tools", [
      {
        type: "tool_proposal",
        proposalId: "read",
        name: "read",
        arguments: { path: "data" },
      },
    ]),
  );
  await until(
    () =>
      r.getSession(id).tools[JSON.stringify(["tools", "read"])]?.status ===
      "pending",
  );
  assert.equal(
    (await r.restore(id, 1)).tools[JSON.stringify(["tools", "read"])].status,
    "pending",
  );
  unwrap(
    await host.advance(fixtureCaller, id, "tools", [
      {
        type: "tool_result",
        proposalId: "read",
        disposition: "rejected",
        text: "not allowed",
      },
      { type: "terminal", outcome: "completed" },
    ]),
  );
  await until(
    () =>
      r.getSession(id).tools[JSON.stringify(["tools", "read"])].status ===
      "failed",
  );
  assert.deepEqual(
    (await r.restore(id, 1)).tools[JSON.stringify(["tools", "read"])],
    r.getSession(id).tools[JSON.stringify(["tools", "read"])],
  );
  assert.equal(
    r.getSession(id).tools[JSON.stringify(["tools", "read"])].result.text,
    "not allowed",
  );
});

// PR 1049: each case exercises a public seam with an adversarial but valid owner.
test("F1 access rejects capability escalation by Host selection", async (t) => {
  const { host, service } = setup(t);
  const { interactionCatalog } = await import(
    "../../packages/ai-contract/dist/index.js"
  );
  host.negotiate = (offer) => ({
    ok: true,
    value: { ...offer, a2ui: interactionCatalog },
  });
  const [a, b] = localTransportPair();
  service.connect(a, fixtureCaller);
  const r = new RuntimeClient(b, { a2ui: false });
  t.after(() => r.close());
  await assert.rejects(r.initialize());
});

for (const extended of [false, true])
  test(`F3 direct ${extended ? "extension" : "standard"} resume replaces the active generation subscription`, async (t) => {
    const { host, service } = setup(t);
    host.createSession = (caller, _options, budget) =>
      host.openSessionForTest(caller, options, budget);
    const { extension } = await import(
      "../../packages/ai-contract/dist/index.js"
    );
    const subscriptions = [];
    const original = host.subscribe.bind(host);
    host.subscribe = async function* (...args) {
      subscriptions.push({ after: args[2], signal: args[3].signal });
      // The spy owns the new watermark; keep the underlying empty fixture alive.
      yield* original(args[0], args[1], 0, args[3]);
    };
    const r = extended ? await runtime(t, service) : undefined;
    const agent = r?.connection.agent ?? (await standard(t, service));
    const id = r
      ? (await r.createSession()).namespace.sessionId
      : (await agent.request("session/new", { cwd: "/", mcpServers: [] }))
          .sessionId;
    await until(() => subscriptions.length === 1);
    host.resume = async (caller, sessionId, budget) => {
      assert.equal(subscriptions[0].signal.aborted, true);
      const s = unwrap(
        await host.snapshotPage(caller, sessionId, { limit: 1 }, budget),
      ).session;
      return {
        ok: true,
        value: replaceStage(
          { ...s, lastSequence: 12 },
          { ...activeStage(s).binding, generation: "new-generation" },
          undefined,
        ),
      };
    };
    await agent.request(
      extended ? extension.resume : "session/resume",
      extended
        ? { schemaVersion: 5, kind: "resumeRequest", sessionId: id }
        : { sessionId: id, cwd: "/", mcpServers: [] },
    );
    await until(() => subscriptions.length === 2);
    assert.equal(subscriptions[1].after, 12);
    assert.equal(subscriptions[1].signal.aborted, false);
  });

test("F4 close settles by its independent cutoff when a subscription ignores cancellation", async (t) => {
  const codes = [];
  const { host, service } = setup(t, {
    shutdownTimeoutMs: 20,
    onDiagnostic: (code) => codes.push(code),
  });
  let release, signal;
  host.subscribe = async function* (...args) {
    signal = args[3].signal;
    await new Promise((resolve) => {
      release = resolve;
    });
  };
  const agent = await standard(t, service);
  await agent.request("session/new", { cwd: "/", mcpServers: [] });
  await until(() => release);
  const close = service.close();
  assert.equal(signal.aborted, true);
  const outcome = await Promise.race([
    close.then(() => "closed"),
    delay(100).then(() => "hung"),
  ]);
  release(); // Test always releases the hostile fixture, including the red case.
  await close;
  assert.equal(outcome, "closed");
  assert.deepEqual(codes, ["cleanup_timeout"]);
});

test("F5 restore accepts projections before their matching event history across pages", async (t) => {
  const { MemorySessionStore, seedSurface } = await import(
    "../../packages/ai-contract/dist/testing/index.js"
  );
  const store = new MemorySessionStore();
  await seedSurface(store);
  const host = new FakeHost(store);
  const snapshot = host.snapshotPage.bind(host);
  host.snapshotPage = async (caller, id, query, budget) => {
    const result = await snapshot(caller, id, { limit: 256 }, budget);
    if (!result.ok) return result;
    const page = result.value;
    return {
      ok: true,
      value: query.continuation
        ? {
            ...page,
            snapshotId: "reversed",
            pageIndex: 1,
            commands: [],
            interactions: [],
            surfaces: [],
          }
        : {
            ...page,
            snapshotId: "reversed",
            pageIndex: 0,
            events: [],
            next: "history",
          },
    };
  };
  const { service } = setup(t, { host });
  const r = await runtime(t, service);
  const view = await r.restore("session-1");
  assert.equal(view.connection, "attached");
  assert.equal(Object.keys(view.surfaces).length, 1);
});

test("F6 sequential complete single-page reads never exhaust the live view quota", async () => {
  const { MemorySessionStore, fixtureSession } = await import(
    "../../packages/ai-contract/dist/testing/index.js"
  );
  const store = new MemorySessionStore({ clock: { now: () => 0 } });
  const s = fixtureSession();
  unwrap(await store.create(s));
  for (let i = 0; i < 140; i++) {
    assert.equal(
      (await store.snapshotPage(s.namespace, { limit: 1 })).ok,
      true,
      `snapshot ${i}`,
    );
    assert.equal(
      (await store.listSessions(fixtureCaller, { limit: 1 })).ok,
      true,
      `list ${i}`,
    );
  }
});

test("F8 observer failures do not corrupt restore or block other observers", async (t) => {
  const { service } = setup(t);
  const [a, b] = localTransportPair();
  service.connect(a, fixtureCaller);
  const codes = [],
    received = [];
  const r = new RuntimeClient(b, {
    onDiagnostic: (code) => {
      codes.push(code);
      throw new Error("diagnostic listener");
    },
  });
  t.after(() => r.close());
  await r.initialize();
  r.observe(() => {
    throw new Error("private observer payload");
  });
  r.observe((view) => received.push(view));
  const view = await r.createSession();
  assert.equal(view.connection, "attached");
  assert.equal(received.at(-1).connection, "attached");
  assert.deepEqual(codes, ["observer_failed"]);
});

test("F2 runtime catalog identity is generated from canonical schema constants", async () => {
  const { readFile } = await import("node:fs/promises");
  const schema = JSON.parse(
    await readFile(
      new URL(
        "../../packages/ai-contract/schema/runtime.schema.json",
        import.meta.url,
      ),
      "utf8",
    ),
  );
  const { interactionCatalog } = await import(
    "../../packages/ai-contract/dist/index.js"
  );
  for (const [key, value] of Object.entries(interactionCatalog))
    assert.equal(schema.$defs.A2uiNegotiation.properties[key].const, value);
  assert.equal(Object.isFrozen(interactionCatalog), true);
});

test("F7 public client and transport failures have closed codes without private causes", async (t) => {
  const api = await import("../../packages/ai-client/dist/index.js");
  const [a, b] = localTransportPair();
  const r = new RuntimeClient(b);
  t.after(() => r.close());
  await assert.rejects(
    r.createSession(),
    (error) => error.code === "not_initialized",
  );
  const stream = channelStream({
    listen() {
      return () => {};
    },
    async send() {
      throw new Error("secret transport payload");
    },
  });
  const writer = stream.writable.getWriter();
  await assert.rejects(
    writer.write({ jsonrpc: "2.0", method: "hello" }),
    (error) => {
      assert.ok(error instanceof api.ClientError);
      assert.equal(error.code, "transport_failed");
      assert.equal("cause" in error, false);
      return !error.message.includes("secret");
    },
  );
});

test("F9 snapshot and live interaction projections preserve authoritative expiry and callback lifetime", async (t) => {
  const { host, service } = setup(t);
  const r = await runtime(t, service);
  const id = (await r.createSession()).namespace.sessionId;
  await r.submit({
    schemaVersion: 5,
    kind: "command",
    sessionId: id,
    commandId: "question",
    expiresAtMs: 1000,
    input: { type: "prompt", policy: "queue_next", text: "question" },
  });
  const interaction = unwrap(
    await host.ask(fixtureCaller, id, "question", { question: "Answer?" }),
  );
  await until(() => r.getSession(id).interactions[interaction.interactionId]);
  const live = r.getSession(id).interactions[interaction.interactionId];
  assert.equal(live.expiresAtMs, interaction.expiresAtMs);
  assert.equal(live.callbackLifetime, interaction.callbackLifetime);
  assert.deepEqual(
    (await r.restore(id, 1)).interactions[interaction.interactionId],
    live,
  );
});

test("F10 answered projection identifies the first response across live and restore", async (t) => {
  const { MemorySessionStore, seedSurface } = await import(
    "../../packages/ai-contract/dist/testing/index.js"
  );
  const store = new MemorySessionStore(),
    seeded = await seedSurface(store);
  const host = new FakeHost(store),
    { service } = setup(t, { host });
  const r = await runtime(t, service),
    id = seeded.session.namespace.sessionId;
  await r.restore(id);
  unwrap(
    await host.respond(
      fixtureCaller,
      {
        schemaVersion: 5,
        kind: "command",
        sessionId: id,
        commandId: "winning-response",
        expiresAtMs: 100,
        input: {
          type: "respond",
          interactionId: seeded.interaction.interactionId,
          generation: seeded.interaction.generation,
          nativeRunId: seeded.interaction.nativeRunId,
          surface: {
            instanceId: seeded.surface.surfaceInstanceId,
            revision: 0,
          },
          answer: { answer: "a" },
        },
      },
      { signal: AbortSignal.timeout(1000), timeoutMs: 1000 },
    ),
  );
  await until(
    () =>
      r.getSession(id).interactions[seeded.interaction.interactionId].status ===
      "answered",
  );
  const live = r.getSession(id).interactions[seeded.interaction.interactionId];
  assert.equal(live.responseCommandId, "winning-response");
  assert.deepEqual(
    (await r.restore(id, 1)).interactions[seeded.interaction.interactionId],
    live,
  );
});

test("F3 detach while resume is pending cannot resurrect the old attachment", async (t) => {
  const { extension } = await import(
    "../../packages/ai-contract/dist/index.js"
  );
  const { host, service } = setup(t),
    r = await runtime(t, service);
  const id = (await r.createSession()).namespace.sessionId;
  let release;
  host.resume = async (caller, sessionId, budget) => {
    await new Promise((resolve) => {
      release = resolve;
    });
    return {
      ok: true,
      value: unwrap(
        await host.snapshotPage(caller, sessionId, { limit: 1 }, budget),
      ).session,
    };
  };
  const result = r.connection.agent.request(extension.resume, {
    schemaVersion: 5,
    kind: "resumeRequest",
    sessionId: id,
  });
  const rejection = assert.rejects(result, /unavailable/);
  await until(() => release);
  await r.detach(id);
  release();
  await rejection;
  assert.equal(r.getSession(id).connection, "detached");
});

test("F1 client independently rejects a server which re-enables an unoffered extension", async (t) => {
  const { agent } = await import(require.resolve("@agentclientprotocol/sdk"));
  const { extension, interactionCatalog } = await import(
    "../../packages/ai-contract/dist/index.js"
  );
  const [a, b] = localTransportPair();
  const server = agent()
    .onRequest("initialize", ({ params }) => ({
      protocolVersion: 1,
      agentCapabilities: {
        _meta: {
          [extension.capability]: {
            ...params.clientCapabilities._meta[extension.capability],
            a2ui: interactionCatalog,
          },
        },
      },
    }))
    .connect(a);
  t.after(() => server.close());
  const r = new RuntimeClient(b, { a2ui: false });
  t.after(() => r.close());
  await assert.rejects(r.initialize(), (e) => e.code === "negotiation_failed");
});

test("F1 selection may disable offered booleans, never enable unoffered ones", async () => {
  const { selectNegotiation, accessLimits } = await import(
    "../../packages/ai-contract/dist/index.js"
  );
  const offer = {
    contractVersion: 5,
    acp: 1,
    cursorAttach: false,
    durableReceipts: false,
  };
  for (const key of ["cursorAttach", "durableReceipts"])
    assert.throws(() =>
      selectNegotiation(offer, { ...offer, [key]: true }, accessLimits),
    );
  assert.deepEqual(
    selectNegotiation(
      { ...offer, cursorAttach: true, durableReceipts: true },
      offer,
      accessLimits,
    ),
    offer,
  );
});

test("Host budget expiry and exceptions project a closed ACP error", async (t) => {
  const { host, service } = setup(t, { timeoutMs: 20 });
  const agent = await standard(t, service);
  let observed;
  for (const mode of ["timeout", "throw"]) {
    host.listSessions = (_caller, _query, budget) => {
      observed = budget.signal;
      if (mode === "throw") throw new Error("private Host detail");
      return new Promise(() => {});
    };
    await assert.rejects(agent.request("session/list", {}), (error) => {
      assert.equal(error.code, -32001);
      assert.equal(error.data.code, "unavailable");
      assert.doesNotMatch(
        error.message,
        /private Host detail|budget exhausted/,
      );
      return true;
    });
    assert.equal(observed.aborted, true);
  }
});
