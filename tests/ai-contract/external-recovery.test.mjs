import { readSnapshot } from "../../packages/ai-contract/dist/testing/index.js";
import assert from "node:assert/strict";
import test from "node:test";
import {
  MemorySessionStore,
  fixtureSession,
  seedSurface,
  seedInteraction,
  dispatchCommand,
  commandCommit,
  fixtureCommand,
  restoredSession,
  emptyCommit,
  terminalCommit,
  interactionEvent,
  acceptance,
  unwrap,
} from "../../packages/ai-contract/dist/testing/index.js";

test("terminal requires atomic closure of pending interactions and active surfaces", async () => {
  const store = new MemorySessionStore();
  const seeded = await seedSurface(store),
    head = seeded.session;
  const record = unwrap(
    await store.command(head.namespace, seeded.interaction.commandId),
  );
  const terminal = await terminalCommit(head, record);
  assert.equal((await store.commit(terminal)).ok, false);
  assert.deepEqual(unwrap(await store.session(head.namespace)), head);
  const unavailable = { ...seeded.interaction, status: "unavailable" };
  const invalidated = {
    ...seeded.surface,
    status: "invalidated",
    revision: seeded.surface.revision + 1,
  };
  const event = {
    ...interactionEvent(head, unavailable),
    eventId: "terminal-unavailable",
    sequence: terminal.session.lastSequence + 1,
  };
  const surfaceEvent = {
    ...event,
    eventId: "terminal-surface",
    sequence: event.sequence + 1,
    body: {
      type: "surface",
      surface: invalidated,
    },
  };
  unwrap(
    await store.commit({
      ...terminal,
      session: { ...terminal.session, lastSequence: surfaceEvent.sequence },
      interactions: [unavailable],
      surfaces: [invalidated],
      events: [...terminal.events, event, surfaceEvent],
    }),
  );
  const after = unwrap(await readSnapshot(store, head.namespace));
  assert.equal(after.interactions[0].status, "unavailable");
  assert.equal(after.surfaces[0].status, "invalidated");
  assert.equal(
    (await store.accept(acceptance(after.session, seeded.answer))).ok,
    false,
  );
});

test("interaction expiry requires time strictly after its inclusive deadline", async () => {
  const store = new MemorySessionStore(),
    seeded = await seedInteraction(store);
  const expired = { ...seeded.interaction, status: "expired" };
  const batch = {
    ...emptyCommit(seeded.session),
    session: {
      ...seeded.session,
      revision: seeded.session.revision + 1,
      lastSequence: seeded.session.lastSequence + 1,
    },
    interactions: [expired],
    events: [interactionEvent(seeded.session, expired)],
  };
  for (const nowMs of [
    undefined,
    -1,
    NaN,
    expired.expiresAtMs - 1,
    expired.expiresAtMs,
  ])
    assert.equal((await store.commit({ ...batch, nowMs })).ok, false);
  unwrap(await store.commit({ ...batch, nowMs: expired.expiresAtMs + 1 }));
});

test("rebind refuses malformed seeds before hashing them into event ids", async () => {
  const store = new MemorySessionStore(),
    initial = fixtureSession();
  initial.capabilities.continuation = "across_processes";
  unwrap(await store.create(initial));
  for (const eventId of ["", "bad space", "x".repeat(129)]) {
    const restored = await restoredSession(initial, "new-generation");
    const result = await store.rebind({
      namespace: initial.namespace,
      expectedRevision: 0,
      expectedGeneration: initial.binding.generation,
      restored,
      eventId,
    });
    assert.equal(result.ok, false);
    assert.equal(result.error.code, "invalid_input");
  }
});

test("raw reconciliation cannot reset an ambiguous attempt", async () => {
  const store = new MemorySessionStore(),
    initial = fixtureSession();
  unwrap(await store.create(initial));
  unwrap(await store.accept(acceptance(initial)));
  const { session, record } = await dispatchCommand(
    store,
    unwrap(await store.session(initial.namespace)),
    "command-1",
    "unknown",
  );
  const accepted = {
    schemaVersion: 4,
    kind: "commandRecord",
    command: record.command,
    receipt: record.receipt,
    state: "accepted",
  };
  const batch = commandCommit(session, accepted, [
    {
      type: "reconciled",
      attempt: record.dispatch,
      resolution: "not_submitted",
    },
  ]);
  batch.events[0].attemptId = record.dispatch.attemptId;
  const result = await store.commit({
    ...batch,
    nowMs: 1,
    providerFacts: [
      {
        commandId: record.command.commandId,
        attemptId: record.dispatch.attemptId,
        binding: session.binding,
        status: "not_submitted",
      },
    ],
  });
  assert.equal(result.ok, false);
  assert.deepEqual(unwrap(await store.session(session.namespace)), session);
});
