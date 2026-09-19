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
  accountRef: "account",
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
    sessionOptions: options,
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
      await host.createSession(fixtureCaller, options, {
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
    (e) => e === failure,
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
          contractVersion: 2,
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
  let generation, oldSignal;
  const snapshot = host.snapshotPage.bind(host),
    subscribe = host.subscribe.bind(host);
  host.snapshotPage = async (...args) => {
    const result = await snapshot(...args);
    if (result.ok && generation)
      result.value.session.binding.generation = generation;
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
    schemaVersion: 2,
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
    schemaVersion: 2,
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
  await until(() => r.getSession(id).tools["tools/read"]?.status === "pending");
  assert.equal((await r.restore(id, 1)).tools["tools/read"].status, "pending");
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
  await until(() => r.getSession(id).tools["tools/read"].status === "failed");
  assert.deepEqual(
    (await r.restore(id, 1)).tools["tools/read"],
    r.getSession(id).tools["tools/read"],
  );
  assert.equal(r.getSession(id).tools["tools/read"].result.text, "not allowed");
});
