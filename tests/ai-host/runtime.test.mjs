import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createHost } from "../../packages/ai-host/dist/index.js";
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
      store,
      resolve: async (caller, options, namespace) => ({
        configuration: {
          namespace,
          provider: options.provider,
          config: options.config,
          accountRef: options.accountRef,
          workingDirectory: directory,
          permissions: extras.admission ? "host_mediated" : "tools_disabled",
        },
        artifact: new URL("./provider.mjs", import.meta.url).href,
        ...(extras.admission ? { admission: extras.admission } : {}),
      }),
      ...extras.hostOptions,
    }),
  );
  t.after(async () => {
    unwrap(await host.close(budget()));
    await rm(directory, { recursive: true, force: true });
  });
  const session = unwrap(await host.createSession(caller, options, budget()));
  const command = (id, text = "hold") => ({
    schemaVersion: 2,
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
      generation: f.session.binding.generation,
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
          generation: f.session.binding.generation,
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
      generation: f.session.binding.generation,
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
  unwrap(await f.host.submit(caller, f.command("flood", "flood"), budget()));
  const abort = new AbortController(),
    iterator = f.host
      .subscribe(caller, f.session.namespace.sessionId, 0, {
        timeoutMs: 5000,
        signal: abort.signal,
      })
      [Symbol.asyncIterator]();
  await iterator.next();
  await new Promise((resolve) => setTimeout(resolve, 700));
  assert.equal((await iterator.next()).value.type, "resync_required");
  abort.abort();
  await iterator.return();
  const second = unwrap(
    await f.host.createSession(
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
    (await f.host.createSession(caller, f.options, budget())).error.code,
    "limit_exceeded",
  );
  unwrap(
    await f.host.createSession(
      caller,
      { ...f.options, accountRef: "second" },
      budget(),
    ),
  );
  assert.equal(
    (
      await f.host.createSession(
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
  assert.equal(verifications[0].tools, admission.tools);
  assert.deepEqual(calls[0].caller, caller);
  assert.equal(calls[0].proposal.name, "fixture");
});
test("shared Host conformance runs against SQLite and a real isolated worker", async (t) => {
  const f = await setup(t, "1", { hostOptions: { now: () => 0 } });
  await runHostConformance(
    () => f.host,
    () => budget(),
  );
});
test("real Host and A04 recover the same stable client projection after detach", async (t) => {
  const f = await setup(t),
    service = createAccessService({ host: f.host, sessionOptions: f.options }),
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
