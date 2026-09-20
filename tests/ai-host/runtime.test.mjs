import { activeStage } from "../../packages/ai-contract/dist/index.js";
import { openFixture, fixtureArtifact } from "./harness.mjs";
import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createRequire } from "node:module";
import { createHost } from "../../packages/ai-host/dist/index.js";
import { groupEmpty } from "../../packages/ai-host/dist/process.js";
import { openSqliteStore } from "../../packages/ai-store-sqlite/dist/index.js";
import { runHostConformance } from "../../packages/ai-contract/dist/testing/index.js";
import { createAccessService } from "../../packages/ai-access/dist/index.js";
import {
  RuntimeClient,
  localTransportPair,
} from "../../packages/ai-client/dist/index.js";
const caller = {
  tenantId: "tenant",
  principalId: "person",
  authorityId: "authority",
};
const require = createRequire(
  new URL("../../packages/ai-access/package.json", import.meta.url),
);
const { client: acpClient } = await import(
  require.resolve("@agentclientprotocol/sdk")
);
async function standardPeer(t, f, accessOptions = {}) {
  const service = createAccessService({
    host: f.host,
    sessionOptions: { connectionId: f.session.selectedConnectionId },
    ...accessOptions,
  });
  const [a, b] = localTransportPair(),
    updates = [];
  service.connect(a, caller);
  const connection = acpClient()
    .onNotification("session/update", ({ params }) => updates.push(params))
    .connect(b);
  t.after(async () => {
    connection.close();
    await service.close();
  });
  await connection.agent.request("initialize", {
    protocolVersion: 1,
    clientCapabilities: {},
  });
  await connection.agent.request("session/load", {
    sessionId: f.session.namespace.sessionId,
    cwd: "/",
    mcpServers: [],
  });
  return { agent: connection.agent, updates };
}
const budget = (timeoutMs = 5000) => ({
  timeoutMs,
  signal: new AbortController().signal,
});
const unwrap = (result) => {
  assert.equal(result.ok, true, JSON.stringify(result));
  return result.value;
};
async function until(check) {
  const deadline = Date.now() + 4000;
  while (Date.now() < deadline) {
    const result = await check();
    if (result) return result;
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
  assert.fail("condition timed out");
}
async function setup(t, revision = "1", extras = {}) {
  const directory = await mkdtemp(join(tmpdir(), "rss-host-"));
  let host;
  const store = unwrap(
    await openSqliteStore({
      path: join(directory, "host.sqlite"),
      mode: "create",
    }),
  );
  const options = {
    provider: "fake",
    config: { id: "config", revision },
    accountRef: "account",
    profile: extras.admission ? "controlled_tools" : "conversation",
  };
  host = unwrap(
    await createHost({
      delivery: extras.admission
        ? {
            prepare: () => ({
              ok: true,
              value: { operationId: "fixture-delivery", target: "fixture" },
            }),
            send: async (request, b) => {
              const saved = unwrap(
                await store.delivery(request.namespace, "fixture-delivery"),
              );
              assert.equal(saved.delivery.status, "reconciliation_required");
              const reply = await extras.admission.tools.propose(
                request.body.proposal,
                b,
              );
              return reply.ok
                ? {
                    ok: true,
                    value: {
                      receiptRef: "fixture-receipt",
                      reply: reply.value,
                    },
                  }
                : reply;
            },
            reconcile: async () => ({ ok: true, value: { state: "unknown" } }),
            acknowledge: async () => ({ ok: true, value: undefined }),
          }
        : null,
      store,
      launchFences: store,
      resolve: async (caller, options, namespace) => ({
        configuration: {
          namespace,
          provider: options.provider,
          config: options.config,
          accountRef: options.accountRef,
          workingDirectory: directory,
          permissions: extras.admission ? "host_mediated" : "tools_disabled",
        },
        artifact: await fixtureArtifact(store, namespace, options),
        ...(extras.admission ? { admission: extras.admission } : {}),
      }),
      ...extras.hostOptions,
    }),
  );
  t.after(async () => {
    unwrap(await host.close(budget()));
    await rm(directory, { recursive: true, force: true });
  });
  const session = unwrap(
    await openFixture(host, store, caller, options, budget()),
  );
  const command = (id, text = "hold") => ({
    schemaVersion: 5,
    kind: "command",
    sessionId: session.namespace.sessionId,
    commandId: id,
    expiresAtMs: Date.now() + 60000,
    input: { type: "prompt", policy: "queue_next", text },
  });
  const record = async (id) =>
    unwrap(await store.command(session.namespace, id));
  const trace = async () =>
    (await readFile(join(directory, "trace.ndjson"), "utf8"))
      .trim()
      .split("\n")
      .map((line) => JSON.parse(line));
  return { host, store, session, command, record, trace, directory, options };
}
test("real worker keeps a long run, durable FIFO and control acknowledgement independent", async (t) => {
  const f = await setup(t);
  unwrap(await f.host.submit(caller, f.command("long"), budget()));
  await until(async () => (await f.record("long")).state === "running");
  unwrap(await f.host.submit(caller, f.command("next", "quick"), budget()));
  assert.equal((await f.record("next")).state, "accepted");
  const long = await f.record("long");
  const cancel = {
    ...f.command("cancel"),
    input: {
      type: "cancel",
      generation: activeStage(f.session).binding.generation,
      targetCommandId: "long",
      nativeRunId: long.dispatch.nativeRunId,
    },
  };
  const started = Date.now();
  unwrap(await f.host.cancel(caller, cancel, budget()));
  await until(async () => (await f.record("cancel")).state === "acknowledged");
  assert.ok(Date.now() - started < 1000);
  await until(async () => (await f.record("next")).state === "terminal");
  assert.equal((await f.record("long")).outcome, "cancelled");
  assert.deepEqual(
    (await f.trace())
      .filter((row) => row.type === "dispatch")
      .map((row) => row.commandId),
    ["long", "cancel", "next"],
  );
  assert.equal(unwrap(await f.store.launches()).length, 1);
});
test("native queued ACK stays dispatching and queued cancellation never calls provider", async (t) => {
  const f = await setup(t, "1", { hostOptions: { queueLimit: 1 } });
  unwrap(
    await f.host.submit(caller, f.command("native", "queued-native"), budget()),
  );
  await until(
    async () => (await f.record("native")).dispatch?.certainty === "submitted",
  );
  assert.equal((await f.record("native")).state, "dispatching");
  unwrap(await f.host.submit(caller, f.command("queued"), budget()));
  unwrap(
    await f.host.cancel(
      caller,
      {
        ...f.command("cancel-queued"),
        input: {
          type: "cancel",
          generation: activeStage(f.session).binding.generation,
          targetCommandId: "queued",
        },
      },
      budget(),
    ),
  );
  await until(async () => (await f.record("queued")).state === "cancelled");
  assert.equal((await f.record("cancel-queued")).state, "acknowledged");
  assert.equal((await f.record("cancel-queued")).dispatch, undefined);
  assert.deepEqual(
    (await f.trace())
      .filter((row) => row.type === "dispatch")
      .map((row) => row.commandId),
    ["native"],
  );
});
test("caller scope, duplicate receipt and window detach preserve execution", async (t) => {
  const f = await setup(t),
    command = f.command("run");
  const receipt = unwrap(await f.host.submit(caller, command, budget()));
  assert.deepEqual(
    unwrap(await f.host.submit(caller, command, budget())),
    receipt,
  );
  const controller = new AbortController(),
    iterator = f.host
      .subscribe(caller, f.session.namespace.sessionId, 0, {
        timeoutMs: 1000,
        signal: controller.signal,
      })
      [Symbol.asyncIterator]();
  assert.equal((await iterator.next()).value.type, "event");
  controller.abort();
  await iterator.return();
  await until(async () => (await f.record("run")).state === "running");
  assert.equal(
    (
      await f.host.snapshotPage(
        { ...caller, principalId: "other" },
        command.sessionId,
        { limit: 256 },
        budget(),
      )
    ).ok,
    false,
  );
  assert.equal(
    unwrap(
      await f.host.listSessions(
        { ...caller, principalId: "other" },
        { limit: 256 },
        budget(),
      ),
    ).items.length,
    0,
  );
});
test("unknown transport submission is durable and never automatically resent", async (t) => {
  const f = await setup(t, "unknown");
  unwrap(await f.host.submit(caller, f.command("uncertain"), budget()));
  await until(
    async () =>
      (await f.record("uncertain")).state === "reconciliation_required",
  );
  const before = await f.record("uncertain");
  assert.equal(before.dispatch.correlationId, "native-correlation");
  unwrap(await f.host.submit(caller, f.command("waiting"), budget()));
  await new Promise((resolve) => setTimeout(resolve, 100));
  assert.equal((await f.record("waiting")).state, "accepted");
  assert.equal(
    (await f.trace()).filter((row) => row.type === "dispatch").length,
    1,
  );
});
test("standard ACP exposes unknown delivery without inventing a terminal or resending", async (t) => {
  const f = await setup(t, "unknown"),
    { agent, updates } = await standardPeer(t, f);
  await assert.rejects(
    agent.request("session/prompt", {
      sessionId: f.session.namespace.sessionId,
      prompt: [{ type: "text", text: "unknown" }],
    }),
    /reconciliation_required/,
  );
  const snapshot = unwrap(
    await f.store.snapshotPage(f.session.namespace, { limit: 64 }),
  );
  assert.equal(snapshot.commands[0].state, "reconciliation_required");
  await until(() =>
    updates.some((row) =>
      row.update.content?.text?.includes("delivery is uncertain"),
    ),
  );
  assert.equal(
    (await f.trace()).filter((row) => row.type === "dispatch").length,
    1,
  );
});
test("standard ACP queued input outlives the request budget and executes in FIFO order", async (t) => {
  let now = 0;
  const f = await setup(t, "1", { hostOptions: { now: () => now } }),
    { agent } = await standardPeer(t, f, { now: () => now, timeoutMs: 1000 });
  const prompt = (text) =>
    agent.request("session/prompt", {
      sessionId: f.session.namespace.sessionId,
      prompt: [{ type: "text", text }],
    });
  const first = prompt("hold");
  void first.catch(() => {});
  const active = await until(async () =>
    unwrap(
      await f.store.snapshotPage(f.session.namespace, { limit: 64 }),
    ).commands.find((row) => row.state === "running"),
  );
  const second = prompt("quick");
  void second.catch(() => {});
  await until(async () =>
    unwrap(
      await f.store.snapshotPage(f.session.namespace, { limit: 64 }),
    ).commands.some(
      (row) => row.command.input.text === "quick" && row.state === "accepted",
    ),
  );
  now = 31000;
  const session = unwrap(await f.store.session(f.session.namespace));
  unwrap(
    await f.host.cancel(
      caller,
      {
        schemaVersion: 5,
        kind: "command",
        sessionId: session.namespace.sessionId,
        commandId: "release-long-run",
        expiresAtMs: now + 5000,
        input: {
          type: "cancel",
          targetCommandId: active.command.commandId,
          generation: activeStage(session).binding.generation,
          nativeRunId: active.dispatch.nativeRunId,
        },
      },
      budget(),
    ),
  );
  assert.deepEqual(await first, { stopReason: "cancelled" });
  assert.deepEqual(await second, { stopReason: "end_turn" });
});
test("question response is timely during a long run and has its own acknowledgement", async (t) => {
  const f = await setup(t);
  unwrap(
    await f.host.submit(caller, f.command("asking", "question"), budget()),
  );
  await until(
    async () =>
      unwrap(await f.store.snapshotPage(f.session.namespace, { limit: 256 }))
        .interactions.length === 1,
  );
  const response = {
    ...f.command("answer"),
    input: {
      type: "respond",
      generation: activeStage(f.session).binding.generation,
      interactionId: "question-1",
      nativeRunId: (await f.record("asking")).dispatch.nativeRunId,
      answer: { text: "yes" },
    },
  };
  const started = Date.now();
  unwrap(await f.host.respond(caller, response, budget()));
  await until(async () => (await f.record("answer")).state === "acknowledged");
  assert.ok(Date.now() - started < 1000);
  assert.equal((await f.record("asking")).state, "running");
});
test("slow subscriber is asked to resync while another account remains usable", async (t) => {
  const f = await setup(t);
  const abort = new AbortController(),
    iterator = f.host
      .subscribe(caller, f.session.namespace.sessionId, 0, {
        timeoutMs: 5000,
        signal: abort.signal,
      })
      [Symbol.asyncIterator]();
  t.after(() => abort.abort());
  const ready = iterator.next();
  // A draining peer proves the Host has published enough bytes to overflow the paused peer.
  // No assumption about how many provider callbacks fit in 700 ms under parallel CI load.
  const fast = f.host
    .subscribe(caller, f.session.namespace.sessionId, 0, {
      timeoutMs: 5000,
      signal: AbortSignal.any([abort.signal, AbortSignal.timeout(5000)]),
    })
    [Symbol.asyncIterator]();
  const fastReady = fast.next();
  unwrap(await f.host.submit(caller, f.command("flood", "flood"), budget()));
  await Promise.all([ready, fastReady]);
  let bytes = 0;
  for await (const item of fast) {
    assert.notEqual(item.type, "resync_required", "draining peer remains live");
    if (item.type === "delta") bytes += Buffer.byteLength(item.text);
    if (bytes >= 2 * 1024 * 1024) break;
  }
  assert.ok(
    bytes >= 2 * 1024 * 1024,
    `Host published ${bytes} bytes, exceeding the paused peer's budget`,
  );
  assert.equal((await iterator.next()).value.type, "resync_required");
  abort.abort();
  await iterator.return();
  const second = unwrap(
    await openFixture(
      f.host,
      f.store,
      caller,
      { ...f.options, accountRef: "separate-account" },
      budget(),
    ),
  );
  const command = {
    ...f.command("isolated", "quick"),
    sessionId: second.namespace.sessionId,
  };
  unwrap(await f.host.submit(caller, command, budget()));
  await until(
    async () =>
      unwrap(await f.store.command(second.namespace, "isolated")).state ===
      "terminal",
  );
});
test("worker quotas bind provider/account and count real process owners", async (t) => {
  const f = await setup(t, "1", {
    hostOptions: { workerLimit: 2, accountWorkerLimit: 1 },
  });
  assert.equal(
    (await openFixture(f.host, f.store, caller, f.options, budget())).error
      .code,
    "limit_exceeded",
  );
  unwrap(
    await openFixture(
      f.host,
      f.store,
      caller,
      { ...f.options, accountRef: "second" },
      budget(),
    ),
  );
  assert.equal(
    (
      await openFixture(
        f.host,
        f.store,
        caller,
        { ...f.options, accountRef: "third" },
        budget(),
      )
    ).error.code,
    "limit_exceeded",
  );
  assert.equal(unwrap(await f.store.launches()).length, 2);
});
test("worker reverse tool RPC reaches the parent-admitted endpoint without serializing a verifier", async (t) => {
  const calls = [],
    verifications = [];
  const admission = {
    tools: {
      propose: async (proposal) => {
        calls.push({ caller, proposal });
        return {
          ok: true,
          value: { disposition: "rejected", text: "fixture" },
        };
      },
    },
    verifier: {
      verify: async (session, tools) => {
        verifications.push({ session, tools });
        return {
          ok: true,
          value: { platform: "fixture", verificationRef: "proof" },
        };
      },
    },
  };
  const f = await setup(t, "1", { admission });
  unwrap(await f.host.submit(caller, f.command("tool", "quick"), budget()));
  await until(() => calls.length === 1);
  assert.equal(typeof verifications[0].tools.propose, "function");
  assert.deepEqual(calls[0].caller, caller);
  assert.equal(calls[0].proposal.name, "fixture");
});
test("worker tool bridge stays closed through factory/session creation and parent verification", async (t) => {
  let calls = 0;
  const admission = {
    tools: {
      propose: async () => {
        calls++;
        return {
          ok: true,
          value: { disposition: "rejected", text: "fixture" },
        };
      },
    },
    verifier: {
      verify: async () => {
        assert.equal(calls, 0);
        return {
          ok: true,
          value: { platform: "fixture", verificationRef: "proof" },
        };
      },
    },
  };
  const f = await setup(t, "early_tool", { admission });
  assert.equal(calls, 0);
  assert.deepEqual(
    (await f.trace())
      .filter((row) => row.type === "early-tool-result")
      .map((row) => row.ok),
    [false, false],
  );
  unwrap(
    await f.host.submit(caller, f.command("tool-after", "quick"), budget()),
  );
  await until(() => calls === 1);
  admission.verifier.verify = async () => ({
    ok: false,
    error: { code: "permission_denied", retry: "never" },
  });
  const rejected = await openFixture(
    f.host,
    f.store,
    caller,
    { ...f.options, accountRef: "rejected" },
    budget(),
  );
  assert.equal(rejected.ok, false);
  assert.equal(calls, 1);
});
test("Host close drains admission without reading a session that is not yet persisted", async (t) => {
  const f = await setup(t);
  const reserve = f.store.reserveLaunch.bind(f.store);
  let release,
    entered = false;
  const held = new Promise((resolve) => {
    release = resolve;
  });
  f.store.reserveLaunch = async (input) => {
    entered = true;
    await held;
    return reserve(input);
  };
  const admission = openFixture(
    f.host,
    f.store,
    caller,
    { ...f.options, accountRef: "closing" },
    budget(),
  );
  await until(() => entered);
  const closing = f.host
    .close(budget(100))
    .catch((error) => ({ thrown: error }));
  await new Promise((resolve) => setTimeout(resolve, 150));
  release();
  const [admitted, closed] = await Promise.all([admission, closing]);
  assert.equal(closed.thrown, undefined);
  assert.equal(admitted.ok, false);
  unwrap(await f.host.close(budget()));
  assert.equal(
    (await f.trace()).filter((row) => row.type === "activate").length,
    1,
  );
});
test("shared Host conformance runs against SQLite and a real isolated worker", async (t) => {
  const f = await setup(t, "1", { hostOptions: { now: () => 0 } });
  const { fixtureCaller } = await import(
    "../../packages/ai-contract/dist/testing/index.js"
  );
  const row = unwrap(
    await f.store.connection(caller, f.session.selectedConnectionId),
  );
  unwrap(
    await f.store.saveConnection(
      fixtureCaller,
      { ...row, connectionId: "cfg" },
      null,
    ),
  );
  await runHostConformance(
    () => f.host,
    () => budget(),
  );
});
test("real Host and A04 recover the same stable client projection after detach", async (t) => {
  const f = await setup(t),
    service = createAccessService({
      host: f.host,
      sessionOptions: { connectionId: f.session.selectedConnectionId },
    }),
    [a, b] = localTransportPair();
  service.connect(a, caller);
  const client = new RuntimeClient(b);
  t.after(async () => {
    await client.close();
    await service.close();
  });
  const selected = await client.initialize();
  assert.equal(selected.durableReceipts, true);
  const view = await client.createSession(),
    id = view.namespace.sessionId;
  const command = { ...f.command("protocol", "quick"), sessionId: id };
  assert.equal((await client.submit(command)).kind, "receipt");
  await until(
    () => client.getSession(id)?.commands.protocol?.state === "terminal",
  );
  const before = client.getSession(id);
  await client.detach(id);
  const recovered = await client.restore(id);
  assert.deepEqual(recovered.commands, before.commands);
  assert.deepEqual(recovered.messages, before.messages);
  assert.ok(
    (await client.listSessions()).items.some(
      (session) => session.namespace.sessionId === id,
    ),
  );
});
test("explicit supported steer is attempt-bound and never starts a competing model turn", async (t) => {
  const f = await setup(t, "steer");
  unwrap(await f.host.submit(caller, f.command("running"), budget()));
  await until(async () => (await f.record("running")).state === "running");
  const target = (await f.record("running")).dispatch.nativeRunId;
  const steer = {
    ...f.command("steer"),
    input: {
      type: "prompt",
      policy: "steer",
      targetRunId: target,
      text: "adjust",
    },
  };
  unwrap(await f.host.submit(caller, steer, budget()));
  await until(async () => (await f.record("steer")).state === "acknowledged");
  assert.equal((await f.record("steer")).acknowledgement.type, "steer");
  assert.equal((await f.record("running")).state, "running");
  assert.equal(
    (
      await f.host.submit(
        caller,
        {
          ...steer,
          commandId: "stale",
          input: { ...steer.input, targetRunId: "other-run" },
        },
        budget(),
      )
    ).ok,
    false,
  );
});

for (const option of [
  "queueLimit",
  "workerLimit",
  "accountWorkerLimit",
  "operationTimeoutMs",
])
  test(`Host factory returns invalid_input for invalid ${option}`, async () => {
    for (const value of [0, -1, NaN, Infinity, 1.5]) {
      const result = await createHost({
        delivery: null,
        store: {},
        resolve: async () => {},
        [option]: value,
      });
      assert.deepEqual(result, {
        ok: false,
        error: { code: "invalid_input", retry: "never" },
      });
    }
  });

for (const method of [
  "snapshotPage",
  "accept",
  "recoverUnavailable",
  "releaseLaunch",
  "close",
])
  test(`Host close bounds a hanging Store ${method} and still terminates the worker`, async (t) => {
    const f = await setup(t);
    unwrap(await f.host.submit(caller, f.command("closing-run"), budget()));
    await until(
      async () => (await f.record("closing-run")).state === "running",
    );
    const pid = (await f.trace()).find((row) => row.type === "activate").pid;
    const original = f.store[method].bind(f.store);
    let release;
    const held = new Promise((resolve) => {
      release = resolve;
    });
    f.store[method] = async (...args) => {
      await held;
      return original(...args);
    };
    try {
      const result = await Promise.race([
        f.host.close(budget(150)),
        new Promise((resolve) =>
          setTimeout(() => resolve("deadline exceeded"), 700),
        ),
      ]);
      assert.notEqual(result, "deadline exceeded");
      assert.equal(result.ok, false);
      assert.equal(result.error.retry, "same_command");
      await until(() => groupEmpty(pid));
    } finally {
      f.store[method] = original;
      release();
    }
  });

for (const fault of ["session_result", "recovery_rejection", "recovery_hang"])
  test(`worker IPC failure isolates the runtime and diagnoses ${fault}`, async (t) => {
    const diagnostics = [];
    const f = await setup(t, "1", {
      hostOptions: {
        operationTimeoutMs: 100,
        onDiagnostic: (d) => diagnostics.push(d),
      },
    });
    const runtime = [...f.host.runtimes.values()][0];
    const pid = (await f.trace()).find((row) => row.type === "activate").pid;
    const method =
      fault === "session_result" ? "session" : "recoverUnavailable";
    const original = f.store[method].bind(f.store);
    let release;
    const held = new Promise((resolve) => {
      release = resolve;
    });
    f.store[method] = async (...args) => {
      if (fault === "session_result")
        return {
          ok: false,
          error: { code: "storage_corrupt", retry: "never" },
        };
      if (fault === "recovery_rejection")
        throw new Error("private storage detail");
      await held;
      return original(...args);
    };
    try {
      // Break a real worker IPC channel while the OS process is still alive.
      runtime.worker.control.close();
      await until(() => diagnostics.some((d) => d.stage === "recovery"));
      await until(() => groupEmpty(pid));
      assert.equal(runtime.abort.signal.aborted, true);
      assert.equal(
        (await f.host.submit(caller, f.command("after-failure"), budget())).ok,
        false,
      );
      assert.equal(
        (await f.trace()).filter((row) => row.type === "activate").length,
        1,
      );
    } finally {
      f.store[method] = original;
      release();
    }
  });

test("client restore keeps recovery_required visible on an attached real Host session", async (t) => {
  const f = await setup(t),
    service = createAccessService({
      host: f.host,
      sessionOptions: { connectionId: f.session.selectedConnectionId },
    });
  const [a, b] = localTransportPair();
  service.connect(a, caller);
  const client = new RuntimeClient(b);
  t.after(async () => {
    await client.close();
    await service.close();
  });
  await client.initialize();
  const id = f.session.namespace.sessionId;
  await client.restore(id);
  const runtime = [...f.host.runtimes.values()][0];
  runtime.worker.control.close();
  await until(
    async () =>
      unwrap(await f.store.session(f.session.namespace)).status ===
      "recovery_required",
  );
  const view = await client.restore(id);
  assert.equal(view.connection, "attached");
  assert.equal(view.sessionStatus, "recovery_required");
  await assert.rejects(client.submit(f.command("unavailable")));
});
