import assert from "node:assert/strict";
import { test } from "node:test";
import { decode } from "../../packages/ai-contract/dist/index.js";
import {
  MemorySessionStore,
  fixtureSession,
  fixtureCommand,
  fixtureCaller,
  fixtureLimits,
  acceptance,
  unwrap,
  fixtures,
} from "../../packages/ai-contract/dist/testing/index.js";

test("snapshot pages freeze one revision while writes continue and bind their caller", async () => {
  const store = new MemorySessionStore();
  const initial = fixtureSession();
  unwrap(await store.create(initial));
  unwrap(await store.accept(acceptance(initial)));
  const page = unwrap(
    await store.snapshotPage(initial.namespace, { limit: 1 }),
  );
  assert.equal(page.cursor, 1);
  assert.ok(page.next);
  const current = unwrap(await store.session(initial.namespace));
  unwrap(
    await store.accept(
      acceptance(current, { ...fixtureCommand(), commandId: "second" }),
    ),
  );
  assert.equal(
    (
      await store.snapshotPage(
        { ...initial.namespace, principalId: "other" },
        { limit: 1, continuation: page.next },
      )
    ).ok,
    false,
  );
  const next = unwrap(
    await store.snapshotPage(initial.namespace, {
      limit: 1,
      continuation: page.next,
    }),
  );
  assert.equal(next.snapshotId, page.snapshotId);
  assert.equal(next.cursor, page.cursor);
  assert.equal(next.session.revision, page.session.revision);
  assert.equal(next.next, undefined);
  assert.equal(
    unwrap(await store.snapshotPage(initial.namespace, { limit: 10 })).cursor,
    2,
  );
  assert.equal(typeof store.snapshot, "undefined");
});

test("session listing is caller scoped and continuation cannot cross authorities", async () => {
  const store = new MemorySessionStore();
  for (const sessionId of ["a", "b", "c"]) {
    unwrap(
      await store.create({
        ...fixtureSession(),
        namespace: { ...fixtureCaller, sessionId },
      }),
    );
  }
  unwrap(
    await store.create({
      ...fixtureSession(),
      namespace: {
        ...fixtureCaller,
        principalId: "other",
        sessionId: "private",
      },
    }),
  );
  const first = unwrap(await store.listSessions(fixtureCaller, { limit: 2 }));
  assert.deepEqual(
    first.items.map((s) => s.namespace.sessionId),
    ["a", "b"],
  );
  const second = unwrap(
    await store.listSessions(fixtureCaller, {
      limit: 2,
      continuation: first.next,
    }),
  );
  assert.deepEqual(
    second.items.map((s) => s.namespace.sessionId),
    ["c"],
  );
  assert.equal(
    (
      await store.listSessions(
        { ...fixtureCaller, authorityId: "other" },
        { limit: 2, continuation: first.next },
      )
    ).ok,
    false,
  );
});

test("surface recovery content is required and ambiguous terminal outcomes are rejected", () => {
  const surface = structuredClone(
    fixtures.valid.find((v) => v.kind === "surface"),
  );
  delete surface.messages;
  assert.throws(() => decode(JSON.stringify(surface), fixtureLimits));
  const event = structuredClone(fixtures.valid.find((v) => v.kind === "event"));
  for (const outcome of ["interrupted", "limit_reached"]) {
    event.body = { type: "terminal", outcome };
    assert.throws(() => decode(JSON.stringify(event), fixtureLimits));
  }
});

test("continuations expire, cannot be forged, and pages honor envelope budgets for long history", async () => {
  let now = 0;
  const store = new MemorySessionStore({
    clock: { now: () => now },
    snapshotTtlMs: 10,
  });
  const session = fixtureSession();
  unwrap(await store.create(session));
  for (let i = 0; i < 3; i++) {
    const head = unwrap(await store.session(session.namespace));
    const command = {
      ...fixtureCommand(`long-${i}`),
      input: { type: "prompt", policy: "queue_next", text: "x".repeat(60000) },
    };
    unwrap(await store.accept(acceptance(head, command)));
  }
  const first = unwrap(
    await store.snapshotPage(session.namespace, { limit: 64 }),
  );
  assert.ok(first.next);
  assert.equal(
    (
      await store.snapshotPage(session.namespace, {
        limit: 64,
        continuation: `${first.next}:1`,
      })
    ).error.code,
    "cursor_expired",
  );
  const commands = [...first.commands];
  let page = first;
  while (page.next) {
    const next = unwrap(
      await store.snapshotPage(session.namespace, {
        limit: 64,
        continuation: page.next,
      }),
    );
    assert.equal(next.pageIndex, page.pageIndex + 1);
    decode(JSON.stringify(next), fixtureLimits);
    commands.push(...next.commands);
    page = next;
  }
  assert.equal(commands.length, 3);
  now = 11;
  assert.equal(
    (
      await store.snapshotPage(session.namespace, {
        limit: 64,
        continuation: first.next,
      })
    ).error.code,
    "cursor_expired",
  );
});
