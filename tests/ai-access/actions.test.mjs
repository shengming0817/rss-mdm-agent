import assert from "node:assert/strict";
import { test } from "node:test";
import { createAccessService } from "../../packages/ai-access/dist/index.js";
import {
  RuntimeClient,
  localTransportPair,
} from "../../packages/ai-client/dist/index.js";
import { validateSurface } from "../../packages/ai-contract/dist/index.js";
import {
  FakeHost,
  MemorySessionStore,
  fixtureCaller,
  seedSurface,
  surfaceCommit,
  emptyCommit,
  unwrap,
} from "../../packages/ai-contract/dist/testing/index.js";
const options = {
  provider: "fake",
  accountRef: "account",
  config: { id: "cfg", revision: "1" },
  profile: "conversation",
};
async function fixture(t) {
  const store = new MemorySessionStore(),
    seeded = await seedSurface(store);
  const host = new FakeHost(store),
    service = createAccessService({
      host,
      sessionOptions: options,
      now: () => 0,
    });
  t.after(() => service.close());
  async function connect(caller = fixtureCaller, a2ui = true) {
    const [a, b] = localTransportPair();
    service.connect(a, caller);
    const runtime = new RuntimeClient(b, { a2ui });
    await runtime.initialize();
    t.after(() => runtime.close());
    return runtime;
  }
  const action = (commandId = "answer") => ({
    schemaVersion: 2,
    kind: "actionRequest",
    expiresAtMs: 100,
    metadata: {
      schemaVersion: 2,
      kind: "surfaceAction",
      sessionId: seeded.session.namespace.sessionId,
      commandId,
      interactionId: seeded.surface.interactionId,
      surfaceInstanceId: seeded.surface.surfaceInstanceId,
      surfaceRevision: 0,
      generation: seeded.surface.generation,
      nativeRunId: seeded.surface.nativeRunId,
    },
    message: {
      version: "v0.9.1",
      action: {
        name: "respond",
        surfaceId: seeded.surface.surfaceId,
        sourceComponentId: seeded.surface.sourceComponentId,
        timestamp: "2026-09-19T00:00:00Z",
        context: { answer: "a", approved: true },
      },
    },
  });
  return { ...seeded, store, host, service, connect, action };
}
test("two clients recover real surface content; only one answer wins and retry returns the original receipt", async (t) => {
  const f = await fixture(t),
    clients = await Promise.all([f.connect(), f.connect()]);
  const views = await Promise.all(
    clients.map((c) => c.restore("session-1", 1)),
  );
  assert.deepEqual(
    views[0].surfaces[f.surface.surfaceInstanceId].messages,
    f.surface.messages,
  );
  const results = await Promise.allSettled(
    clients.map((c, i) => c.action(f.action(`answer-${i}`))),
  );
  assert.equal(results.filter((r) => r.status === "fulfilled").length, 1);
  assert.match(
    results.find((r) => r.status === "rejected").reason.message,
    /already_answered|revision_conflict/,
  );
  const winner = results.findIndex((r) => r.status === "fulfilled");
  assert.deepEqual(
    await clients[winner].action(f.action(`answer-${winner}`)),
    results[winner].value,
  );
  const row = unwrap(
    await f.store.command(f.session.namespace, `answer-${winner}`),
  );
  assert.equal(row.command.input.answer.approved, true); // still untrusted answer data, no execution permit
});
test("actions reject stale revisions, old schema, cross caller/session, missing negotiation, expiry and deleted cards", async (t) => {
  const f = await fixture(t),
    runtime = await f.connect();
  await runtime.restore("session-1");
  for (const patch of [
    { surfaceRevision: 7 },
    { generation: "old" },
    { nativeRunId: "other" },
    { interactionId: "other" },
    { sessionId: "other" },
  ]) {
    const action = f.action();
    Object.assign(action.metadata, patch);
    await assert.rejects(runtime.action(action));
  }
  const old = f.action();
  old.message.version = "v0.9";
  await assert.rejects(runtime.action(old));
  const other = await f.connect({ ...fixtureCaller, principalId: "other" });
  await assert.rejects(other.action(f.action()));
  const plain = await f.connect(fixtureCaller, false);
  await assert.rejects(plain.action(f.action()), /not negotiated/);
  const current = unwrap(await f.store.session(f.session.namespace));
  const deleted = {
    ...f.surface,
    revision: 1,
    status: "deleted",
    messages: [
      ...f.surface.messages,
      { version: "v0.9.1", deleteSurface: { surfaceId: f.surface.surfaceId } },
    ],
  };
  unwrap(
    await f.store.commit(
      surfaceCommit(current, deleted, f.interaction.commandId),
    ),
  );
  f.host.notify(f.session.namespace);
  await assert.rejects(runtime.action(f.action()));
  const restored = await runtime.restore("session-1", 1);
  assert.equal(
    restored.interactions[f.interaction.interactionId].status,
    "unavailable",
  );
});
test("catalog rejects unknown components, functions, unsafe paths and invalid lifecycle before rendering", async (t) => {
  const f = await fixture(t);
  assert.equal(validateSurface(f.surface).status, "active");
  const invalids = [
    (s) => (s.catalogId = "unknown"),
    (s) => (s.messages[0].version = "v0.9"),
    (s) => (s.messages[1].updateComponents.components[0].component = "Image"),
    (s) => (s.messages[1].updateComponents.components[0].children = ["root"]),
    (s) => (s.messages[2].updateDataModel.path = "/__proto__/owned"),
    (s) =>
      (s.messages[1].updateComponents.components[1].text = {
        call: "eval",
        args: { value: "script" },
      }),
    (s) => s.messages.reverse(),
    (s) => (s.messages[0].createSurface.sendDataModel = true),
  ];
  for (const mutate of invalids) {
    const s = structuredClone(f.surface);
    mutate(s);
    assert.throws(() => validateSurface(s));
  }
});

test("expired or lost question callback stays unavailable after display recovery", async (t) => {
  const f = await fixture(t),
    runtime = await f.connect();
  await runtime.restore("session-1");
  f.host.clock.now = () => 101;
  await assert.rejects(
    runtime.action({ ...f.action(), expiresAtMs: 1000 }),
    /expired/,
  );
  assert.equal(
    (await runtime.restore("session-1")).interactions[
      f.interaction.interactionId
    ].status,
    "pending",
  );
  await assert.rejects(
    runtime.action({ ...f.action("after-restore"), expiresAtMs: 1000 }),
    /expired/,
  );
  f.host.clock.now = () => 0;
  const head = unwrap(await f.store.session(f.session.namespace));
  unwrap(
    await f.store.commit({
      ...emptyCommit(head),
      session: {
        ...head,
        revision: head.revision + 1,
        lastSequence: head.lastSequence + 1,
      },
      interactions: [{ ...f.interaction, status: "unavailable" }],
      events: [
        {
          schemaVersion: 2,
          kind: "event",
          namespace: head.namespace,
          eventId: "callback-lost",
          sequence: head.lastSequence + 1,
          commandId: f.interaction.commandId,
          generation: f.interaction.generation,
          body: {
            type: "interaction",
            interactionId: f.interaction.interactionId,
            status: "unavailable",
          },
        },
      ],
    }),
  );
  f.host.notify(head.namespace);
  assert.equal(
    (await runtime.restore("session-1")).interactions[
      f.interaction.interactionId
    ].status,
    "unavailable",
  );
  await assert.rejects(runtime.action(f.action("lost")), /unavailable/);
});
test("invalid surface never advances the client cursor; replacing it permits snapshot recovery", async (t) => {
  const f = await fixture(t),
    runtime = await f.connect();
  const original = await runtime.restore("session-1");
  const bad = { ...f.surface, revision: 1, catalogId: "unknown" };
  // A faulty Host implementation is deliberately injected at its public read boundary.
  const read = f.host.snapshotPage.bind(f.host);
  f.host.snapshotPage = async (...args) => {
    const result = await read(...args);
    if (result.ok) result.value.surfaces = [bad];
    return result;
  };
  await assert.rejects(runtime.restore("session-1"));
  assert.equal(runtime.getSession("session-1").cursor, original.cursor);
  f.host.snapshotPage = read;
  assert.equal((await runtime.restore("session-1")).connection, "attached");
});
