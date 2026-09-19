import assert from "node:assert/strict";
import { VerifiedProviderSession } from "../session.js";
import type { Binding, CommandRecord } from "../wire.js";
import type { ProviderAgentPort, SessionStore } from "../ports.js";
import { defaultBudget } from "./budget.js";
import {
  fixtureSession,
  fixtureCommand,
  acceptance,
  unwrap,
  emptyCommit,
  dispatchCommand,
  commandCommit,
  terminalCommit,
  seedSurface,
} from "./conformance.js";

/** Scripted admission proof, never actual provider/process restoration evidence. */
export async function restoredSession(previous: Binding, generation: string) {
  const binding = { ...previous, generation };
  const configuration = {
    provider: previous.provider,
    config: previous.config,
    accountRef: previous.accountRef,
    workingDirectory: ".",
    permissions: "tools_disabled" as const,
  };
  const port = {
    resume: async () => ({
      ok: true as const,
      value: {
        binding,
        capabilities: {
          ...fixtureSession().capabilities,
          continuation: "across_processes" as const,
        },
      },
    }),
  } as unknown as ProviderAgentPort;
  return unwrap(
    await VerifiedProviderSession.restore(
      port,
      previous,
      configuration,
      defaultBudget(),
    ),
  );
}

/** The same recovery transitions must pass for memory and every durable adapter. */
export async function runRecoveryConformance(
  create: () => Promise<SessionStore>,
) {
  const store = await create();
  const initial = fixtureSession();
  initial.capabilities.continuation = "across_processes";
  unwrap(await store.create(initial));
  unwrap(await store.accept(acceptance(initial)));
  let head = unwrap(await store.session(initial.namespace));
  const unknown = await dispatchCommand(store, head, "command-1", "unknown");
  head = unknown.session;
  const original = unknown.record.dispatch!;
  const control = {
    ...fixtureCommand("cancel-1"),
    input: {
      type: "cancel" as const,
      targetCommandId: "command-1",
      generation: head.binding.generation,
    },
  };
  unwrap(await store.accept(acceptance(head, control)));
  head = unwrap(await store.session(initial.namespace));
  const oldHead = head;
  const restored = await restoredSession(head.binding, "generation-2");
  const input = {
    namespace: head.namespace,
    expectedRevision: head.revision,
    expectedGeneration: head.binding.generation,
    restored,
    eventId: "rebind-1",
  };
  assert.equal(
    (
      await store.rebind({
        ...input,
        restored: JSON.parse(JSON.stringify(restored)),
      })
    ).ok,
    false,
    "wire cannot mint restore evidence",
  );
  head = unwrap(await store.rebind(input));
  assert.equal(head.revision, oldHead.revision + 1);
  assert.equal((await store.commit(emptyCommit(oldHead))).ok, false);
  const rebound = unwrap(await store.command(head.namespace, "command-1"));
  assert.deepEqual(rebound.dispatch, {
    ...original,
    observerGeneration: "generation-2",
  });
  assert.equal(rebound.state, "reconciliation_required");
  assert.equal(
    unwrap(await store.command(head.namespace, "cancel-1")).state,
    "invalidated",
  );
  assert.equal((await store.rebind(input)).ok, false);
  const late = terminalCommit(head, rebound);
  assert.equal(
    (
      await store.commit({
        ...late,
        events: late.events.map((e) => ({
          ...e,
          generation: oldHead.binding.generation,
        })),
      })
    ).ok,
    false,
  );
  const unknownAgain = commandCommit(head, rebound, [
    { type: "reconciled", attempt: rebound.dispatch!, resolution: "unknown" },
  ]);
  unwrap(
    await store.commit({
      ...unknownAgain,
      reconciliations: [
        {
          commandId: "command-1",
          attemptId: original.attemptId,
          binding: head.binding,
          status: "unknown",
        },
      ],
    }),
  );
  head = unwrap(await store.session(head.namespace));
  assert.equal(
    (await store.retire(head.namespace, head.revision, head.binding.generation))
      .ok,
    false,
  );
  const accepted: CommandRecord = {
    schemaVersion: 2,
    kind: "commandRecord",
    command: rebound.command,
    receipt: rebound.receipt,
    state: "accepted",
  };
  const retryBase = commandCommit(head, accepted, [
    {
      type: "reconciled",
      attempt: rebound.dispatch!,
      resolution: "not_submitted",
    },
    { type: "status", state: "accepted" },
  ]);
  // Reconciliation events retain the attempt even though the next state has none.
  const retry = {
    ...retryBase,
    events: retryBase.events.map((e, i) =>
      i === 0 ? ({ ...e, attemptId: original.attemptId } as typeof e) : e,
    ),
  };
  const proof = {
    commandId: "command-1",
    attemptId: original.attemptId,
    binding: head.binding,
    status: "not_submitted" as const,
  };
  assert.equal(
    (await store.commit({ ...retry, nowMs: 1 })).ok,
    false,
    "no reset without positive evidence",
  );
  assert.equal(
    (
      await store.commit({
        ...retry,
        nowMs: 1,
        reconciliations: [{ ...proof, attemptId: "other" }],
      })
    ).ok,
    false,
  );
  unwrap(await store.commit({ ...retry, nowMs: 1, reconciliations: [proof] }));
  head = unwrap(await store.session(head.namespace));
  const nextAttempt = {
    ...original,
    attemptId: "attempt-second",
    originGeneration: head.binding.generation,
    observerGeneration: head.binding.generation,
    certainty: "intent" as const,
  };
  delete nextAttempt.correlationId;
  const dispatching: CommandRecord = {
    ...accepted,
    state: "dispatching",
    dispatch: nextAttempt,
  };
  const start = commandCommit(head, dispatching, [
    { type: "dispatch", attempt: nextAttempt },
    { type: "status", state: "dispatching" },
  ]);
  unwrap(await store.commit(start));
  head = unwrap(await store.session(head.namespace));
  const terminal = terminalCommit(head, dispatching);
  assert.equal(
    (
      await store.commit({
        ...terminal,
        events: terminal.events.map(
          (e) => ({ ...e, attemptId: original.attemptId }) as typeof e,
        ),
      })
    ).ok,
    false,
    "late callback from the previous attempt",
  );
  unwrap(await store.commit(terminal));
  head = unwrap(await store.session(head.namespace));
  const third = await restoredSession(head.binding, "generation-3");
  head = unwrap(
    await store.rebind({
      ...input,
      expectedRevision: head.revision,
      expectedGeneration: head.binding.generation,
      restored: third,
      eventId: "rebind-3",
    }),
  );
  const reused = await restoredSession(head.binding, "generation-1");
  assert.equal(
    (
      await store.rebind({
        ...input,
        expectedRevision: head.revision,
        expectedGeneration: head.binding.generation,
        restored: reused,
        eventId: "rebind-reuse",
      })
    ).ok,
    false,
  );
  const history = unwrap(await store.snapshot(head.namespace, 1024));
  const resolved = history.events.find(
    (e) =>
      e.body.type === "reconciled" && e.body.resolution === "not_submitted",
  );
  assert.equal(resolved?.body.type, "reconciled");
  if (resolved?.body.type === "reconciled")
    assert.equal(resolved.body.attempt.correlationId, original.correlationId);
  assert.deepEqual(
    unwrap(await store.events(head.namespace, oldHead.lastSequence, 1024)),
    history.events.filter((e) => e.sequence > oldHead.lastSequence),
  );

  // Display state survives; live callbacks and controls do not survive rebind.
  const surfaceStore = await create();
  const seeded = await seedSurface(surfaceStore, initial);
  const before = unwrap(
    await surfaceStore.snapshot(seeded.session.namespace, 1024),
  );
  const restoredSurface = await restoredSession(
    before.session.binding,
    "surface-restored",
  );
  const after = unwrap(
    await surfaceStore.rebind({
      namespace: before.session.namespace,
      expectedRevision: before.session.revision,
      expectedGeneration: before.session.binding.generation,
      restored: restoredSurface,
      eventId: "surface-rebind",
    }),
  );
  const snapshot = unwrap(await surfaceStore.snapshot(after.namespace, 1024));
  assert.equal(snapshot.interactions[0].status, "unavailable");
  assert.equal(snapshot.surfaces[0].status, "invalidated");
  assert.deepEqual(
    snapshot.events.slice(0, before.events.length),
    before.events,
  );
  assert.equal(
    snapshot.events.some((e) => e.body.type === "surface_invalidated"),
    true,
  );
  assert.equal(
    snapshot.events.some(
      (e) => e.body.type === "surface" && e.body.operation === "delete",
    ),
    false,
  );
  assert.equal(
    (await surfaceStore.accept(acceptance(after, seeded.answer))).ok,
    false,
  );
}
