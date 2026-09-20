import assert from "node:assert/strict";
import { test } from "node:test";
import { decode } from "../../packages/ai-contract/dist/index.js";
import {
  MemorySessionStore,
  fixtureSession,
  fixtureCommand,
  fixtureLimits,
  acceptance,
  unwrap,
} from "../../packages/ai-contract/dist/testing/index.js";

test("V4 acceptance is a self-contained stable fact shared by replay and live consumers", async () => {
  const store = new MemorySessionStore(),
    session = fixtureSession(),
    command = fixtureCommand();
  unwrap(await store.create(session));
  const input = acceptance(session, command);
  const receipt = unwrap(await store.accept(input));
  assert.deepEqual(unwrap(await store.accept(input)), receipt);
  const snapshot = unwrap(
    await store.snapshotPage(session.namespace, { limit: 256 }),
  );
  assert.equal(snapshot.events.length, 1);
  const event = snapshot.events[0];
  assert.equal(event.schemaVersion, 4);
  assert.deepEqual(event.body, { type: "command_accepted", command });
  assert.equal("event" in input, false);
  assert.equal(input.eventId, event.eventId);
  assert.throws(() =>
    decode(
      JSON.stringify({ ...event, body: { type: "status", state: "accepted" } }),
      fixtureLimits,
    ),
  );
  assert.throws(() =>
    decode(JSON.stringify({ ...command, schemaVersion: 2 }), fixtureLimits),
  );
  assert.throws(() =>
    decode(
      JSON.stringify({
        ...event,
        body: {
          type: "command_accepted",
          command: { ...command, commandId: "forged" },
        },
      }),
      fixtureLimits,
    ),
  );
  assert.throws(() =>
    decode(
      JSON.stringify({
        ...event,
        body: {
          type: "command_accepted",
          command: { ...command, sessionId: "other-session" },
        },
      }),
      fixtureLimits,
    ),
  );
});

test("acceptance constructs and budgets its own event atomically", async () => {
  const store = new MemorySessionStore(),
    session = fixtureSession();
  unwrap(await store.create(session));
  const invalid = acceptance(session, fixtureCommand());
  invalid.eventId = "invalid id";
  assert.equal((await store.accept(invalid)).ok, false);
  const oversized = acceptance(session, {
    ...fixtureCommand(),
    input: {
      type: "prompt",
      text: "x".repeat(fixtureLimits.maxTextBytes + 1),
      policy: "queue_next",
    },
  });
  assert.equal((await store.accept(oversized)).ok, false);
  const snapshot = unwrap(
    await store.snapshotPage(session.namespace, { limit: 256 }),
  );
  assert.equal(snapshot.session.revision, 0);
  assert.deepEqual(snapshot.events, []);
  assert.deepEqual(snapshot.commands, []);
});
