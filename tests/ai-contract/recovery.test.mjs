import { activeStage } from "../../packages/ai-contract/dist/index.js";
import assert from "node:assert/strict";
import test from "node:test";
import { decode } from "../../packages/ai-contract/dist/index.js";
import {
  MemorySessionStore,
  fixtureSession,
  acceptance,
  emptyCommit,
  seedInteraction,
  fixtureLimits,
  unwrap,
} from "../../packages/ai-contract/dist/testing/index.js";

test("dispatch identity separates the original attempt from its current observer", async () => {
  const store = new MemorySessionStore(),
    session = fixtureSession();
  unwrap(await store.create(session));
  unwrap(await store.accept(acceptance(session)));
  const record = unwrap(await store.command(session.namespace, "command-1"));
  const current = {
    ...record,
    state: "reconciliation_required",
    dispatch: {
      attemptId: "attempt-1",
      originGeneration: activeStage(session).binding.generation,
      observerGeneration: "restored-generation",
      nativeSessionId: activeStage(session).binding.nativeSessionId,
      certainty: "unknown",
      correlationId: "provider-lookup-key",
    },
  };
  assert.deepEqual(decode(JSON.stringify(current), fixtureLimits), current);
});

test("a store exposes an atomic verified rebind operation", () => {
  assert.equal(typeof new MemorySessionStore().rebind, "function");
});

test("local invalidation is a closed final state without fabricated model outcome", async () => {
  const store = new MemorySessionStore(),
    session = fixtureSession();
  unwrap(await store.create(session));
  unwrap(await store.accept(acceptance(session)));
  const record = unwrap(await store.command(session.namespace, "command-1"));
  const invalidated = {
    ...record,
    state: "invalidated",
    failure: { code: "stale_binding", retry: "never" },
  };
  assert.deepEqual(
    decode(JSON.stringify(invalidated), fixtureLimits),
    invalidated,
  );
});

test("surface mutations cannot leave the stable subscription watermark unchanged", async () => {
  const store = new MemorySessionStore(),
    seeded = await seedInteraction(store);
  const surface = {
    schemaVersion: 5,
    kind: "surface",
    namespace: seeded.session.namespace,
    generation: seeded.interaction.generation,
    nativeRunId: seeded.interaction.nativeRunId,
    surfaceId: "surface-1",
    surfaceInstanceId: "instance-1",
    revision: 0,
    interactionId: seeded.interaction.interactionId,
    sourceComponentId: "button-1",
    eventName: "choose",
    catalogId: "basic",
    catalogVersion: "1",
    a2uiVersion: "v0.9.1",
    status: "active",
  };
  const result = await store.commit({
    ...emptyCommit(seeded.session),
    surfaces: [surface],
  });
  assert.equal(result.ok, false);
});
