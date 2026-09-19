import assert from "node:assert/strict";
import test from "node:test";
import { join } from "node:path";
import {
  boundedJson,
  decode,
  validateSurface,
} from "../../packages/ai-contract/dist/index.js";
import {
  acceptance,
  fixtureCommand,
  fixtureLimits,
  fixtureSession,
  readSnapshot,
  restoredSession,
  seedSurface,
  unwrap,
} from "../../packages/ai-contract/dist/testing/index.js";
import { harness, budget } from "./support.mjs";

test("SQLite access pages freeze a cursor across writes, isolate callers, and expire on reopen", async (t) => {
  const h = harness(t),
    path = join(h.directory, "access.sqlite");
  let store = unwrap(h.open(path));
  const seeded = await seedSurface(store);
  const first = unwrap(
    await store.snapshotPage(seeded.session.namespace, { limit: 1 }),
  );
  assert.ok(first.next);
  const head = unwrap(await store.session(seeded.session.namespace));
  unwrap(
    await store.accept(
      acceptance(head, { ...fixtureCommand(), commandId: "after-capture" }),
    ),
  );
  const pages = [first];
  while (pages.at(-1).next) {
    const query = { limit: 1, continuation: pages.at(-1).next };
    const next = unwrap(await store.snapshotPage(head.namespace, query));
    assert.deepEqual(
      unwrap(await store.snapshotPage(head.namespace, query)),
      next,
    );
    assert.equal(next.snapshotId, first.snapshotId);
    assert.equal(next.cursor, first.cursor);
    assert.equal(next.pageIndex, pages.at(-1).pageIndex + 1);
    assert.deepEqual(
      decode(boundedJson(next, fixtureLimits), fixtureLimits),
      next,
    );
    pages.push(next);
  }
  assert.ok(
    !pages
      .flatMap((p) => p.commands)
      .some((c) => c.command.commandId === "after-capture"),
  );
  assert.deepEqual(
    validateSurface(pages.flatMap((p) => p.surfaces)[0]).messages,
    seeded.surface.messages,
  );
  assert.ok(
    unwrap(await store.events(head.namespace, first.cursor, 256)).some(
      (e) => e.commandId === "after-capture",
    ),
  );
  const other = fixtureSession();
  other.namespace = { ...head.namespace, tenantId: "other" };
  unwrap(await store.create(other));
  assert.equal(
    (
      await store.snapshotPage(other.namespace, {
        limit: 1,
        continuation: first.next,
      })
    ).error.code,
    "cursor_expired",
  );
  assert.deepEqual(
    unwrap(await store.listSessions(head.namespace, { limit: 1 })).items.map(
      (s) => s.namespace,
    ),
    [head.namespace],
  );
  unwrap(await store.close(budget()));
  store = unwrap(h.open(path, "open"));
  assert.equal(
    (
      await store.snapshotPage(head.namespace, {
        limit: 1,
        continuation: first.next,
      })
    ).error.code,
    "cursor_expired",
  );
  assert.ok(
    unwrap(await readSnapshot(store, head.namespace)).commands.some(
      (c) => c.command.commandId === "after-capture",
    ),
  );
});

test("SQLite verified restart preserves renderable surface history while revoking its old callback", async (t) => {
  const h = harness(t),
    path = join(h.directory, "rebound-access.sqlite");
  let store = unwrap(h.open(path));
  const initial = fixtureSession();
  initial.capabilities.continuation = "across_processes";
  const seeded = await seedSurface(store, initial);
  unwrap(await store.close(budget()));
  store = unwrap(h.open(path, "open"));
  const restored = await restoredSession(seeded.session, "access-restored");
  const session = unwrap(
    await store.rebind({
      namespace: initial.namespace,
      expectedRevision: seeded.session.revision,
      expectedGeneration: initial.binding.generation,
      restored,
      eventId: "access-rebind",
    }),
  );
  const snapshot = unwrap(await readSnapshot(store, initial.namespace));
  const surface = validateSurface(snapshot.surfaces[0]);
  assert.equal(surface.status, "invalidated");
  assert.equal(surface.generation, initial.binding.generation);
  assert.deepEqual(surface.messages, seeded.surface.messages);
  const event = snapshot.events.find(
    (e) => e.body.type === "surface" && e.body.surface.status === "invalidated",
  );
  assert.equal(event.generation, session.binding.generation);
  assert.deepEqual(
    decode(boundedJson(event, fixtureLimits), fixtureLimits),
    event,
  );
  assert.equal(snapshot.interactions[0].status, "unavailable");
  assert.equal(
    (await store.accept(acceptance(session, seeded.answer))).ok,
    false,
  );
});
