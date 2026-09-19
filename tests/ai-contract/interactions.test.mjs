import assert from "node:assert/strict";
import { test } from "node:test";
import { decode } from "../../packages/ai-contract/dist/index.js";
import {
  MemorySessionStore,
  fixtureSession,
  fixtureCommand,
  fixtureLimits,
  acceptance,
  emptyCommit,
  unwrap,
  dispatchCommand,
} from "../../packages/ai-contract/dist/testing/index.js";

async function dispatched() {
  const store = new MemorySessionStore();
  const initial = fixtureSession();
  unwrap(await store.create(initial));
  unwrap(await store.accept(acceptance(initial)));
  const head = unwrap(await store.session(initial.namespace));
  await dispatchCommand(store, head, "command-1", "submitted", {
    nativeRequestId: "prompt-1",
  });
  return { store, session: unwrap(await store.session(initial.namespace)) };
}
function question(session, id = "question-1", callback = "callback-1") {
  return {
    schemaVersion: 4,
    kind: "interaction",
    category: "question",
    namespace: session.namespace,
    interactionId: id,
    commandId: "command-1",
    generation: session.binding.generation,
    nativeCallbackId: callback,
    expiresAtMs: 100,
    status: "pending",
    callbackLifetime: "generation_bound",
    request: { question: "Select a value", options: ["a", "b"] },
  };
}
function pending(session, rows) {
  return {
    ...emptyCommit(session),
    session: {
      ...session,
      revision: session.revision + 1,
      lastSequence: session.lastSequence + rows.length,
    },
    interactions: rows,
    events: rows.map((row, index) => ({
      schemaVersion: 4,
      kind: "event",
      namespace: session.namespace,
      eventId: `pending-${row.interactionId}`,
      commandId: row.commandId,
      attemptId: "attempt-command-1",
      generation: row.generation,
      sequence: session.lastSequence + index + 1,
      body: {
        type: "interaction",
        interactionId: row.interactionId,
        status: "pending",
        expiresAtMs: row.expiresAtMs,
        callbackLifetime: row.callbackLifetime,
        request: row.request,
      },
    })),
  };
}
test("callback identity replaces the old request field without dual reading", () => {
  const row = question(fixtureSession());
  assert.deepEqual(decode(JSON.stringify(row), fixtureLimits), row);
  const { nativeCallbackId, ...rest } = row;
  assert.throws(() =>
    decode(
      JSON.stringify({ ...rest, nativeRequestId: nativeCallbackId }),
      fixtureLimits,
    ),
  );
  assert.throws(() =>
    decode(
      JSON.stringify({ ...row, nativeRequestId: "parent" }),
      fixtureLimits,
    ),
  );
});
test("one native prompt accepts two distinct callbacks and answers in reverse order", async () => {
  const { store, session } = await dispatched();
  const rows = [
    question(session),
    question(session, "question-2", "callback-2"),
  ];
  unwrap(await store.commit(pending(session, rows)));
  for (const row of rows.toReversed()) {
    const head = unwrap(await store.session(session.namespace));
    const response = {
      ...fixtureCommand(`answer-${row.interactionId}`),
      input: {
        type: "respond",
        interactionId: row.interactionId,
        generation: row.generation,
        answer: { choice: "a" },
      },
    };
    const accepted = acceptance(head, response);
    const receipt = unwrap(await store.accept(accepted));
    assert.deepEqual(unwrap(await store.accept(accepted)), receipt);
    const next = unwrap(await store.session(session.namespace));
    assert.equal(
      (
        await store.accept(
          acceptance(next, {
            ...response,
            commandId: `duplicate-${row.interactionId}`,
          }),
        )
      ).error.code,
      "already_answered",
    );
  }
  const snapshot = unwrap(
    await store.snapshotPage(session.namespace, { limit: 256 }),
  );
  assert.equal(snapshot.interactions.length, 2);
  assert.ok(snapshot.interactions.every((row) => row.status === "answered"));
  assert.deepEqual(snapshot.interactions[0].request, rows[0].request);
});
test("one native callback cannot acquire a second product interaction identity", async () => {
  const { store, session } = await dispatched();
  unwrap(await store.commit(pending(session, [question(session)])));
  const head = unwrap(await store.session(session.namespace));
  assert.equal(
    (await store.commit(pending(head, [question(head, "alias")]))).ok,
    false,
  );
});
test("pending state and its display event commit together with identical payload", async () => {
  for (const mutation of [
    (batch) => {
      batch.events = [];
      batch.session.lastSequence -= 1;
    },
    (batch) => {
      batch.interactions = [];
    },
    (batch) => {
      batch.events[0].body.request = { question: "different" };
    },
    (batch) => {
      batch.events[0].commandId = "unrelated";
    },
  ]) {
    const { store, session } = await dispatched();
    const batch = pending(session, [question(session)]);
    mutation(batch);
    assert.equal((await store.commit(batch)).ok, false);
    assert.equal(
      unwrap(await store.snapshotPage(session.namespace, { limit: 256 }))
        .interactions.length,
      0,
    );
  }
});
test("callback and question content cannot be rebound after publication", async () => {
  const { store, session } = await dispatched();
  const row = question(session);
  unwrap(await store.commit(pending(session, [row])));
  const head = unwrap(await store.session(session.namespace));
  for (const patch of [
    { nativeCallbackId: "replacement" },
    { request: { question: "replacement" } },
    { commandId: "other" },
    { generation: "old" },
  ]) {
    assert.equal(
      (
        await store.commit({
          ...emptyCommit(head),
          interactions: [{ ...row, ...patch }],
        })
      ).ok,
      false,
    );
  }
});

test("wire rejects pending without request and lifecycle events with request", () => {
  const session = fixtureSession(),
    row = question(session);
  const event = pending(session, [row]).events[0];
  const missing = structuredClone(event);
  delete missing.body.request;
  assert.throws(() => decode(JSON.stringify(missing), fixtureLimits));
  for (const status of ["answered", "expired", "unavailable"])
    assert.throws(() =>
      decode(
        JSON.stringify({ ...event, body: { ...event.body, status } }),
        fixtureLimits,
      ),
    );
});

test("wire callback category accepts questions only and is mandatory", () => {
  const row = { ...question(fixtureSession()), category: "question" };
  assert.deepEqual(decode(JSON.stringify(row), fixtureLimits), row);
  for (const category of [undefined, "tool_permission"])
    assert.throws(() =>
      decode(JSON.stringify({ ...row, category }), fixtureLimits),
    );
});

test("pending events cannot disagree with the authoritative deadline or callback lifetime", async () => {
  const { store, session } = await dispatched();
  for (const patch of [
    { expiresAtMs: 101 },
    { callbackLifetime: "provider_resumable" },
  ]) {
    const batch = pending(session, [question(session)]);
    Object.assign(batch.events[0].body, patch);
    assert.equal((await store.commit(batch)).ok, false);
    assert.equal(
      unwrap(await store.snapshotPage(session.namespace, { limit: 256 }))
        .interactions.length,
      0,
    );
  }
});
