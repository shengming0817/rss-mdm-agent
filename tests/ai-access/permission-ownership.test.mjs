import assert from "node:assert/strict";
import { test } from "node:test";
import { getEventListeners } from "node:events";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";
import { createAccessService } from "../../packages/ai-access/dist/index.js";
import { localTransportPair } from "../../packages/ai-client/dist/index.js";
import {
  FakeHost,
  fixtureCaller,
} from "../../packages/ai-contract/dist/testing/index.js";
const require = createRequire(
  new URL("../../packages/ai-access/package.json", import.meta.url),
);
const { client } = await import(require.resolve("@agentclientprotocol/sdk"));
const tick = () => new Promise(setImmediate);
const graph = (signal) => {
  const key = Object.getOwnPropertySymbols(signal).find(
    (s) => s.description === "kDependantSignals",
  );
  return [getEventListeners(signal, "abort").length, signal[key]?.size ?? 0];
};

test("real ACP permission scopes release long-lived source ownership", async (t) => {
  if (!global.gc) {
    const env = { ...process.env };
    delete env.NODE_TEST_CONTEXT;
    const child = spawnSync(
      process.execPath,
      [
        "--expose-gc",
        "--test",
        "--test-name-pattern=^real ACP",
        fileURLToPath(import.meta.url),
      ],
      { env, encoding: "utf8", timeout: 120000 },
    );
    assert.equal(child.status, 0, child.stdout + child.stderr);
    return;
  }
  assert.equal(process.versions.node, "24.14.1");
  const host = new FakeHost();
  const pumps = [];
  const subscribe = host.subscribe.bind(host);
  host.subscribe = (...args) => {
    pumps.push(args[3].signal);
    return subscribe(...args);
  };
  // Observe the service-owned controller without adding a production debug API.
  const NativeController = globalThis.AbortController;
  const owned = [];
  globalThis.AbortController = class extends NativeController {
    constructor() {
      super();
      owned.push(this.signal);
    }
  };
  let service;
  try {
    service = createAccessService({
      host,
      sessionOptions: { connectionId: "cfg" },
      timeoutMs: 1000,
      shutdownTimeoutMs: 100,
    });
  } finally {
    globalThis.AbortController = NativeController;
  }
  t.after(() => service.close());
  const lifetime = owned[0];
  let mode = "allow",
    loserCancelled = 0;
  const connections = [];
  for (let peer = 0; peer < 2; peer++) {
    const [a, b] = localTransportPair();
    service.connect(a, fixtureCaller);
    const connection = client()
      .onNotification("session/update", () => {})
      .onRequest("session/request_permission", async ({ signal }) => {
        if (peer === 0 && mode !== "timeout")
          return { outcome: { outcome: "selected", optionId: mode } };
        if (mode === "invalid") {
          await tick();
          return { outcome: { outcome: "selected", optionId: "allow" } };
        }
        await new Promise((resolve) => {
          if (signal.aborted) resolve();
          else signal.addEventListener("abort", resolve, { once: true });
        });
        loserCancelled++;
        return { outcome: { outcome: "cancelled" } };
      })
      .connect(b);
    connections.push(connection);
    t.after(() => connection.close());
    await connection.agent.request("initialize", {
      protocolVersion: 1,
      clientCapabilities: {},
    });
  }
  const { sessionId } = await connections[0].agent.request("session/new", {
    cwd: "/",
    mcpServers: [],
  });
  await connections[1].agent.request("session/load", {
    sessionId,
    cwd: "/",
    mcpServers: [],
  });
  const caller = new AbortController();
  const sources = [lifetime, caller.signal, ...pumps];
  await tick();
  global.gc();
  const baseline = sources.map(graph);
  const timers = () =>
    process.getActiveResourcesInfo().filter((name) => name === "Timeout")
      .length;
  const initialTimers = timers();
  const request = {
    sessionId,
    toolCall: { toolCallId: "p", title: "Permission" },
    options: [
      { optionId: "allow", kind: "allow_once", name: "Allow" },
      { optionId: "deny", kind: "reject_once", name: "Deny" },
    ],
  };
  for (let i = 0; i < 2; i++) {
    mode = i % 2 ? "deny" : "allow";
    assert.equal(
      (await service.requestPermission(fixtureCaller, request, caller.signal))
        .outcome.optionId,
      mode,
    );
    // Drain SDK cancellation notifications before issuing another request.
    await tick();
    if (i === 1) {
      global.gc();
      assert.deepEqual(
        sources.map(graph),
        baseline,
        `source graph grew after ${i + 1} deliveries`,
      );
      assert.ok(
        timers() <= initialTimers,
        "completed deliveries leave no watchdogs",
      );
    }
  }
  assert.equal(loserCancelled, 2);
  mode = "invalid";
  assert.equal(
    (await service.requestPermission(fixtureCaller, request, caller.signal))
      .outcome.optionId,
    "allow",
    "an invalid first answer cannot win",
  );
  mode = "timeout";
  assert.equal(
    (await service.requestPermission(fixtureCaller, request, caller.signal))
      .outcome.outcome,
    "cancelled",
  );
  await tick();
  global.gc();
  assert.deepEqual(sources.map(graph), baseline);
  assert.ok(timers() <= initialTimers);
  await service.close();
  assert.ok(pumps.every((signal) => signal.aborted));
  assert.equal(getEventListeners(lifetime, "abort").length, 0);
});

test("repeated timeout and detach release permission and Host budgets", async (t) => {
  const { RuntimeClient } = await import(
    "../../packages/ai-client/dist/index.js"
  );
  const host = new FakeHost();
  const pumps = [],
    hostBudgets = [];
  const subscribe = host.subscribe.bind(host);
  host.subscribe = (...args) => {
    pumps.push(args[3].signal);
    return subscribe(...args);
  };
  const snapshot = host.snapshotPage.bind(host);
  host.snapshotPage = async (...args) => {
    hostBudgets.push(args[3].signal);
    return snapshot(...args);
  };
  const service = createAccessService({
    host,
    sessionOptions: { connectionId: "cfg" },
    timeoutMs: 20,
    shutdownTimeoutMs: 100,
  });
  t.after(() => service.close());
  let entered = 0,
    cancelled = 0;
  const clients = [];
  for (let i = 0; i < 2; i++) {
    const [a, b] = localTransportPair();
    service.connect(a, fixtureCaller);
    const runtime = new RuntimeClient(b, {
      requestPermission: async (_request, signal) => {
        entered++;
        await new Promise((resolve) => {
          if (signal.aborted) resolve();
          else signal.addEventListener("abort", resolve, { once: true });
        });
        cancelled++;
        return { outcome: { outcome: "cancelled" } };
      },
    });
    clients.push(runtime);
    t.after(() => runtime.close());
    await runtime.initialize();
  }
  const view = await clients[0].createSession();
  const sessionId = view.namespace.sessionId;
  await clients[1].restore(sessionId);
  const caller = new AbortController();
  const request = {
    sessionId,
    toolCall: { toolCallId: "p", title: "Question" },
    options: [{ optionId: "allow", name: "Allow", kind: "allow_once" }],
  };
  const baseline = pumps.map(graph);
  for (let i = 0; i < 2; i++) {
    assert.equal(
      (await service.requestPermission(fixtureCaller, request, caller.signal))
        .outcome.outcome,
      "cancelled",
    );
    await tick();
    assert.deepEqual(
      pumps.map(graph),
      baseline,
      "timeout leaves pumps at baseline",
    );
    assert.deepEqual(graph(caller.signal), [0, 0]);
  }
  for (let i = 0; i < 2; i++) {
    const before = entered;
    const pending = service.requestPermission(
      fixtureCaller,
      request,
      caller.signal,
    );
    for (let attempt = 0; entered < before + 2 && attempt < 100; attempt++)
      await tick();
    assert.equal(entered, before + 2);
    await Promise.all(clients.map((c) => c.detach(sessionId)));
    assert.equal((await pending).outcome.outcome, "cancelled");
    await tick();
    assert.equal(
      cancelled,
      entered,
      "every detached peer receives cancellation",
    );
    assert.deepEqual(graph(caller.signal), [0, 0]);
    assert.ok(pumps.every((s) => s.aborted));
    assert.ok(
      pumps.every((s) => graph(s).every((n) => n === 0)),
      "detached pumps retain no links",
    );
    pumps.length = 0;
    await Promise.all(clients.map((c) => c.restore(sessionId)));
  }
  assert.ok(hostBudgets.length >= 4);
  assert.ok(
    hostBudgets.every((s) => s.aborted),
    "each Host call ends its own budget scope",
  );
  assert.ok(hostBudgets.every((s) => graph(s).every((n) => n === 0)));
});
