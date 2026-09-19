import assert from "node:assert/strict";
import { test } from "node:test";
import {
  MemorySessionStore,
  seedInteraction,
  seedSurface,
  runStoreConformance,
  fixtureSession,
  fixtureCommand,
  acceptance,
  emptyCommit,
  unwrap,
} from "../../packages/ai-contract/dist/testing/index.js";
test("common store conformance: isolated identities, CAS, unknown dispatch and retention", () =>
  runStoreConformance(() => new MemorySessionStore()));
test("failed acceptance publishes neither inbox nor event; lost receipt returns original", async () => {
  const store = new MemorySessionStore(),
    s = fixtureSession();
  unwrap(await store.create(s));
  store.failNextCommit = true;
  assert.equal((await store.accept(acceptance())).ok, false);
  assert.equal(
    unwrap(await store.snapshotPage(s.namespace, { limit: 256 })).cursor,
    0,
  );
  const a = unwrap(await store.accept(acceptance()));
  assert.deepEqual(unwrap(await store.accept(acceptance())), a);
});
test("pending callback answer is consumed atomically and is unavailable after loss", async () => {
  const store = new MemorySessionStore();
  const { session: head, answer: command } = await seedInteraction(store);
  const s = head;
  const input = acceptance(head, command);
  store.failNextCommit = true;
  assert.equal((await store.accept(input)).ok, false);
  assert.equal(
    unwrap(await store.snapshotPage(s.namespace, { limit: 256 }))
      .interactions[0].status,
    "pending",
  );
  const receipt = unwrap(await store.accept(input));
  assert.deepEqual(unwrap(await store.accept(input)), receipt);
  const next = unwrap(await store.session(s.namespace));
  assert.equal(
    (
      await store.accept(
        acceptance(next, { ...command, commandId: "answer-2" }),
      )
    ).error.code,
    "already_answered",
  );
});
test("snapshot retains stable history and exposes a continuation at its page bound", async () => {
  const store = new MemorySessionStore(),
    s = fixtureSession();
  unwrap(await store.create(s));
  unwrap(await store.accept(acceptance()));
  const snapshot = unwrap(
    await store.snapshotPage(s.namespace, { limit: 256 }),
  );
  assert.equal(snapshot.events.at(-1).sequence, snapshot.cursor);
  assert.ok(unwrap(await store.snapshotPage(s.namespace, { limit: 1 })).next);
});

test("terminal cannot be committed without matching provider evidence", async () => {
  const store = new MemorySessionStore(),
    session = fixtureSession();
  unwrap(await store.create(session));
  unwrap(await store.accept(acceptance(session)));
  const head = unwrap(await store.session(session.namespace));
  const record = unwrap(await store.command(session.namespace, "command-1"));
  assert.equal(
    (
      await store.commit({
        ...emptyCommit(head),
        commands: [{ ...record, state: "terminal", outcome: "completed" }],
      })
    ).ok,
    false,
  );
});

test("paged store errors use Result and all store operations reject after close", async () => {
  const store = new MemorySessionStore();
  assert.deepEqual(await store.recovery(0), {
    ok: false,
    error: { code: "invalid_input", retry: "never" },
  });
  assert.deepEqual(await store.deliveries(0, 0), {
    ok: false,
    error: { code: "invalid_input", retry: "never" },
  });
  assert.equal(typeof store.close, "function");
  unwrap(
    await store.close({
      timeoutMs: 1000,
      signal: new AbortController().signal,
    }),
  );
  assert.deepEqual(await store.recovery(1), {
    ok: false,
    error: { code: "unavailable", retry: "never" },
  });
  assert.equal((await store.create(fixtureSession())).ok, false);
});

test("pagination preserves transient storage failure and retry advice", async () => {
  const store = new MemorySessionStore();
  for (const operation of [
    () => store.recovery(1),
    () => store.deliveries(1, 0),
  ]) {
    store.failNextQuery = true;
    assert.deepEqual(await operation(), {
      ok: false,
      error: { code: "unavailable", retry: "same_command" },
    });
    assert.equal((await operation()).ok, true);
  }
});

test("Store conformance closes failed stores and retains cleanup errors", async () => {
  const primary = new Error("read failed"),
    cleanup = new Error("close failed");
  const store = new MemorySessionStore();
  store.create = async () => {
    throw primary;
  };
  store.close = async () => {
    throw cleanup;
  };
  await assert.rejects(
    runStoreConformance(() => store),
    (error) =>
      error instanceof AggregateError &&
      error.errors[0] === primary &&
      error.errors[1].errors[0] === cleanup,
  );
});

test("surface response and deletion cannot be smuggled into one commit", async () => {
  const store = new MemorySessionStore();
  const seeded = await seedSurface(store);
  const commit = store.commit.bind(store);
  store.commit = (batch) =>
    commit({
      ...batch,
      surfaces: [{ ...seeded.surface, status: "deleted", revision: 1 }],
    });
  const answer = {
    ...seeded.answer,
    input: {
      ...seeded.answer.input,
      surface: { instanceId: seeded.surface.surfaceInstanceId, revision: 0 },
    },
  };
  assert.equal(
    (await store.accept(acceptance(seeded.session, answer))).ok,
    false,
  );
  assert.equal(
    unwrap(await store.snapshotPage(seeded.session.namespace, { limit: 256 }))
      .interactions[0].status,
    "pending",
  );
});
