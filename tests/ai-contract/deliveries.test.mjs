import { activeStage } from "../../packages/ai-contract/dist/index.js";
import assert from "node:assert/strict";
import { test } from "node:test";
import { deliveryFingerprint } from "../../packages/ai-contract/dist/index.js";
import {
  MemorySessionStore,
  fixtureSession,
  acceptance,
  emptyCommit,
  unwrap,
  dispatchCommand,
  fixtureLimits,
  terminalCommit,
} from "../../packages/ai-contract/dist/testing/index.js";

async function setup() {
  const store = new MemorySessionStore();
  let session = fixtureSession();
  unwrap(await store.create(session));
  unwrap(await store.accept(acceptance(session)));
  session = unwrap(await store.session(session.namespace));
  await dispatchCommand(store, session, "command-1", "submitted");
  session = unwrap(await store.session(session.namespace));
  const event = {
    schemaVersion: 5,
    kind: "event",
    namespace: session.namespace,
    eventId: "delivery-request",
    generation: activeStage(session).binding.generation,
    sequence: session.lastSequence + 1,
    commandId: "command-1",
    body: {
      type: "delivery_requested",
      operationId: "submit-request",
      target: "execution",
      proposal: {
        name: "execution_submit",
        arguments: { operationRequestId: "request" },
      },
    },
  };
  const delivery = {
    schemaVersion: 5,
    kind: "delivery",
    namespace: session.namespace,
    operationId: event.body.operationId,
    eventId: event.eventId,
    target: event.body.target,
    contentHash: deliveryFingerprint(event, event.body.target, fixtureLimits),
    retry: "reconcile_first",
    status: "pending",
    attempts: 0,
    nextAttemptAtMs: 0,
  };
  const batch = (head, events, deliveries = [], commands = []) => ({
    ...emptyCommit(head),
    commands,
    events,
    deliveries,
    session: {
      ...head,
      revision: head.revision + 1,
      lastSequence: head.lastSequence + events.length,
    },
  });
  return { store, session, event, delivery, batch };
}

test("Host delivery intent and immutable request must commit together", async () => {
  const f = await setup();
  assert.equal((await f.store.commit(f.batch(f.session, [f.event]))).ok, false);
  unwrap(await f.store.commit(f.batch(f.session, [f.event], [f.delivery])));
  const found = unwrap(
    await f.store.delivery(f.session.namespace, f.delivery.operationId),
  );
  assert.deepEqual(found, { delivery: f.delivery, event: f.event });
});

test("a durable result reference may arrive after the AI command ends, but must match its delivery", async () => {
  const f = await setup();
  unwrap(await f.store.commit(f.batch(f.session, [f.event], [f.delivery])));
  let head = unwrap(await f.store.session(f.session.namespace));
  const record = unwrap(await f.store.command(head.namespace, "command-1"));
  unwrap(await f.store.commit(await terminalCommit(head, record)));
  head = unwrap(await f.store.session(head.namespace));
  const result = {
    ...f.event,
    eventId: "receipt",
    sequence: head.lastSequence + 1,
    body: {
      type: "delivery_recorded",
      operationId: f.delivery.operationId,
      target: f.delivery.target,
      contentHash: f.delivery.contentHash,
      receiptRef: "rust-receipt",
    },
  };
  const settled = { ...f.delivery, status: "receipt_recorded", attempts: 1 };
  assert.equal(
    (
      await f.store.commit(
        f.batch(
          head,
          [{ ...result, body: { ...result.body, operationId: "other" } }],
          [settled],
        ),
      )
    ).ok,
    false,
  );
  unwrap(await f.store.commit(f.batch(head, [result], [settled])));
  assert.equal(
    unwrap(await f.store.command(head.namespace, "command-1")).outcome,
    "completed",
  );
});
