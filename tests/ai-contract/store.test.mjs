import assert from "node:assert/strict";
import { test } from "node:test";
import {
  MemorySessionStore,
  seedInteraction,
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
  assert.equal(unwrap(await store.snapshot(s.namespace)).cursor, 0);
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
    unwrap(await store.snapshot(s.namespace)).interactions[0].status,
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
test("snapshot retains stable history and fails explicitly at its output bound", async () => {
  const store = new MemorySessionStore(),
    s = fixtureSession();
  unwrap(await store.create(s));
  unwrap(await store.accept(acceptance()));
  const snapshot = unwrap(await store.snapshot(s.namespace, 1024));
  assert.equal(snapshot.events.at(-1).sequence, snapshot.cursor);
  assert.equal(
    (await store.snapshot(s.namespace, 1)).error.code,
    "limit_exceeded",
  );
});
