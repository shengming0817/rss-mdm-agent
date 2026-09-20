import { startStage, providerStage, replaceStage } from "../contexts.js";
import { activeStage } from "../contexts.js";
import { readSnapshot } from "./snapshot.js";
import { fingerprint } from "../codec.js";
import { verifiedReconciliation } from "./recovery.js";
import type { VerifiedProviderFact } from "../session.js";
import { workspaceIdentity } from "../session.js";
import assert from "node:assert/strict";
import { runRecoveryConformance } from "./recovery.js";
import {
  withinBudget,
  boundedPort,
  withCleanup,
  closeAll,
  defaultBudget,
  type BudgetFactory,
} from "./budget.js";
import { fixtures } from "./fixtures.js";
import { deliveryFingerprint } from "../codec.js";
import { fixtureLimits } from "./store.js";
import type {
  AcceptCommand,
  Caller,
  SessionStore,
  SessionCommit,
  Result,
} from "../ports.js";
import type {
  Command,
  CommandRecord,
  Session,
  Event,
  DispatchAttempt,
  SurfaceState,
} from "../wire.js";
export const fixtureCaller: Caller = {
  tenantId: "tenant-1",
  principalId: "user-1",
  authorityId: "authority-1",
};
export function fixtureSession(): Session {
  return startStage(
    {
      schemaVersion: 5,
      kind: "session",
      namespace: { ...fixtureCaller, sessionId: "session-1" },
      revision: 0,
      lastSequence: 0,
      status: "active",
      stages: [],
    },
    providerStage(
      {
        workspaceId: workspaceIdentity("."),
        provider: "fake",
        providerVersion: "fixture-1",
        adapterVersion: "fixture-1",
        generation: "generation-1",
        accountRef: "account-1",
        nativeSessionId: "native-1",
        config: { id: "config-1", revision: "1" },
      },
      {
        continuation: "unsupported",
        cancellation: "request_only",
        tools: "disabled",
        steer: "unsupported",
        fork: "unsupported",
        subagent: "unsupported",
        terminal: "unsupported",
        structuredQuestion: "unsupported",
        multimodal: "unsupported",
      },
    ),
  );
}
export function fixtureCommand(id = "command-1"): Command {
  return {
    schemaVersion: 5,
    kind: "command",
    sessionId: "session-1",
    commandId: id,
    expiresAtMs: 1000,
    input: { type: "prompt", text: "fixture text", policy: "queue_next" },
  };
}
export function acceptance(
  session = fixtureSession(),
  command = fixtureCommand(),
): AcceptCommand {
  return {
    namespace: session.namespace,
    command,
    expectedRevision: session.revision,
    expectedGeneration: activeStage(session).binding.generation,
    nowMs: 0,
    retention: { retryWindowMs: 100, receiptWindowMs: 200 },
    eventId: `event-${command.commandId}`,
  };
}
export function unwrap<T>(result: Result<T>): T {
  assert.equal(result.ok, true, JSON.stringify(result));
  if (!result.ok) throw new Error("conformance failed");
  return result.value;
}
export function emptyCommit(session: Session): SessionCommit {
  return {
    namespace: session.namespace,
    expectedRevision: session.revision,
    expectedGeneration: activeStage(session).binding.generation,
    session: { ...structuredClone(session), revision: session.revision + 1 },
    commands: [],
    events: [],
    interactions: [],
    deliveries: [],
    surfaces: [],
  };
}
/** Shared behavior suite; each adapter supplies an isolated store. Real backend durability
 * and process faults must be supplied separately by the adapter's own harness. */
export async function runStoreConformance(
  create: () => Promise<SessionStore> | SessionStore,
  budget: BudgetFactory = defaultBudget,
): Promise<void> {
  const stores: SessionStore[] = [];
  await withCleanup(
    () =>
      runStoreScenarios(async () => {
        const store = await withinBudget(budget, create);
        stores.push(store);
        return boundedPort(store, budget);
      }),
    () => closeAll(stores, budget),
  );
}
async function runStoreScenarios(
  create: () => Promise<SessionStore>,
): Promise<void> {
  await runStoreBoundaries(create);
  await runRecoveryConformance(create);
  await runSurfaceConformance(await create());
  await runCallbackConformance(await create());
  await runPageConformance(await create());
  const store = await create(),
    s = fixtureSession(),
    input = acceptance(s);
  unwrap(await store.create(s));
  const [a, b] = await Promise.all([store.accept(input), store.accept(input)]);
  assert.deepEqual(unwrap(a), unwrap(b));
  assert.equal(
    unwrap(await store.snapshotPage(s.namespace, { limit: 256 })).commands
      .length,
    1,
  );
  const conflict = await store.accept({
    ...input,
    command: {
      ...input.command,
      input: { type: "prompt", text: "different", policy: "queue_next" },
    },
  });
  assert.deepEqual(conflict, {
    ok: false,
    error: { code: "content_conflict", retry: "never" },
  });
  assert.deepEqual(await store.accept({ ...input, nowMs: 201 }), {
    ok: false,
    error: { code: "expired", retry: "never" },
  });
  assert.deepEqual(
    await store.command(
      { ...s.namespace, tenantId: "other" },
      input.command.commandId,
    ),
    { ok: false, error: { code: "session_gone", retry: "never" } },
  );
  const current = unwrap(await store.session(s.namespace));
  assert.deepEqual(
    await store.commit({
      ...emptyCommit(current),
      expectedGeneration: "stale",
    }),
    { ok: false, error: { code: "stale_binding", retry: "never" } },
  );
  assert.deepEqual(await store.commit(emptyCommit(s)), {
    ok: false,
    error: { code: "revision_conflict", retry: "same_command" },
  });
  const accepted = unwrap(
    await store.command(s.namespace, input.command.commandId),
  );
  assert.equal(
    (
      await store.commit({
        ...emptyCommit(current),
        commands: [
          {
            ...accepted,
            state: "terminal",
            outcome: "completed",
          } as CommandRecord,
        ],
      })
    ).ok,
    false,
  );
  assert.deepEqual(await store.recovery(0), {
    ok: false,
    error: { code: "invalid_input", retry: "never" },
  });
  assert.deepEqual(await store.deliveries(0, 0), {
    ok: false,
    error: { code: "invalid_input", retry: "never" },
  });
  const progressed = await dispatchCommand(
    store,
    current,
    "command-1",
    "unknown",
  );
  const unknown = progressed.session,
    record = progressed.record;
  assert.equal(
    unwrap(await store.recovery(10)).items[0].state,
    "reconciliation_required",
  );
  assert.equal(
    (await store.commit({ ...emptyCommit(unknown), commands: [accepted] })).ok,
    false,
    "unknown cannot return to accepted without proof",
  );
  const batch = await terminalCommit(
    unknown,
    record,
    await verifiedReconciliation(unknown, record, "terminal"),
  );
  for (const mutate of [
    (b: SessionCommit) => ({ ...b, events: [] }),
    (b: SessionCommit) => ({
      ...b,
      events: b.events.map((e) => ({ ...e, generation: "stale" })),
    }),
    (b: SessionCommit) => ({ ...b, providerFacts: [] }),
  ])
    assert.equal((await store.commit(mutate(batch))).ok, false);
  unwrap(await store.commit(batch));
  const terminal = unwrap(await store.session(s.namespace));
  unwrap(
    await store.retire(
      s.namespace,
      terminal.revision,
      activeStage(s).binding.generation,
    ),
  );
  assert.equal(unwrap(await store.pruneRetired(200)), 0);
  assert.equal(unwrap(await store.pruneRetired(201)), 1);
  assert.equal((await store.accept(input)).ok, false);
  assert.equal(
    (await store.create(s)).ok,
    false,
    "retired namespace cannot be resurrected",
  );
  unwrap(await store.close(defaultBudget()));
  assert.deepEqual(await store.recovery(1), {
    ok: false,
    error: { code: "unavailable", retry: "never" },
  });
  assert.deepEqual(await store.deliveries(1, 0), {
    ok: false,
    error: { code: "unavailable", retry: "never" },
  });
  assert.equal((await store.create(fixtureSession())).ok, false);
}

/** A continuation is a frozen read view, including writes between any two pages. */
async function runPageConformance(store: SessionStore): Promise<void> {
  const initial = fixtureSession();
  unwrap(await store.create(initial));
  for (let i = 0; i < 3; i++) {
    const head = unwrap(await store.session(initial.namespace));
    unwrap(await store.accept(acceptance(head, fixtureCommand(`frozen-${i}`))));
  }
  const expected = unwrap(
    await store.snapshotPage(initial.namespace, { limit: 256 }),
  );
  const first = unwrap(
    await store.snapshotPage(initial.namespace, { limit: 1 }),
  );
  assert.ok(first.next);
  const other = {
    ...initial,
    namespace: { ...initial.namespace, sessionId: "other-session" },
  };
  unwrap(await store.create(other));
  for (const namespace of [
    other.namespace,
    { ...initial.namespace, principalId: "other" },
  ])
    assert.equal(
      (
        await store.snapshotPage(namespace, {
          limit: 1,
          continuation: first.next,
        })
      ).ok,
      false,
    );
  const collected = {
    commands: [...first.commands],
    events: [...first.events],
    interactions: [...first.interactions],
    surfaces: [...first.surfaces],
  };
  let continuation: string | undefined = first.next,
    index = 1;
  while (continuation) {
    assert.ok(index < 32, "bounded conformance continuation");
    const head = unwrap(await store.session(initial.namespace));
    unwrap(
      await store.accept(
        acceptance(head, fixtureCommand(`concurrent-${index}`)),
      ),
    );
    const page: import("../wire.js").SnapshotPage = unwrap(
      await store.snapshotPage(initial.namespace, { limit: 1, continuation }),
    );
    assert.equal(page.snapshotId, first.snapshotId);
    assert.equal(page.cursor, first.cursor);
    assert.deepEqual(page.session, first.session);
    assert.equal(page.pageIndex, index++);
    collected.commands.push(...page.commands);
    collected.events.push(...page.events);
    collected.interactions.push(...page.interactions);
    collected.surfaces.push(...page.surfaces);
    continuation = page.next;
  }
  assert.deepEqual(collected, {
    commands: expected.commands,
    events: expected.events,
    interactions: expected.interactions,
    surfaces: expected.surfaces,
  });
  assert.ok(
    unwrap(await store.session(initial.namespace)).lastSequence > first.cursor,
  );
}

/** Prepare a real dispatched callback using only the public store port. */
export async function seedInteraction(
  store: SessionStore,
  status: "pending" | "unavailable" = "pending",
  session: Session = fixtureSession(),
) {
  unwrap(await store.create(session));
  unwrap(await store.accept(acceptance(session)));
  const initial = unwrap(await store.session(session.namespace));
  const { session: current } = await dispatchCommand(
    store,
    initial,
    "command-1",
    "submitted",
    { nativeRunId: "run-1", nativeRequestId: "parent-request-1" },
  );
  const interaction: import("../wire.js").Interaction = {
    schemaVersion: 5,
    kind: "interaction",
    category: "question",
    namespace: session.namespace,
    interactionId: "question-1",
    commandId: "command-1",
    generation: activeStage(session).binding.generation,
    nativeCallbackId: "callback-1",
    request: { question: "Choose an option", options: ["a", "b"] },
    nativeRunId: "run-1",
    expiresAtMs: 100,
    status: "pending",
    callbackLifetime: "generation_bound",
  };
  unwrap(
    await store.commit({
      ...emptyCommit(current),
      session: {
        ...current,
        revision: current.revision + 1,
        lastSequence: current.lastSequence + 1,
      },
      interactions: [interaction],
      events: [interactionEvent(current, interaction)],
    }),
  );
  if (status === "unavailable") {
    const head = unwrap(await store.session(session.namespace));
    interaction.status = status;
    unwrap(
      await store.commit({
        ...emptyCommit(head),
        session: {
          ...head,
          revision: head.revision + 1,
          lastSequence: head.lastSequence + 1,
        },
        interactions: [interaction],
        events: [interactionEvent(head, interaction)],
      }),
    );
  }
  const answer: Command = {
    ...fixtureCommand("answer-1"),
    input: {
      type: "respond",
      interactionId: interaction.interactionId,
      generation: interaction.generation,
      nativeRunId: interaction.nativeRunId,
      answer: { choice: "allow" },
    },
  };
  return {
    session: unwrap(await store.session(session.namespace)),
    interaction,
    answer,
  };
}
async function runStoreBoundaries(
  create: () => Promise<SessionStore> | SessionStore,
): Promise<void> {
  const store = await create(),
    s = fixtureSession();
  unwrap(await store.create(s));
  for (const binding of [
    { ...activeStage(s).binding, generation: "other" },
    { ...activeStage(s).binding, provider: "other" },
    { ...activeStage(s).binding, providerVersion: "other" },
    { ...activeStage(s).binding, adapterVersion: "other" },
    { ...activeStage(s).binding, accountRef: "other" },
    { ...activeStage(s).binding, nativeSessionId: "other" },
    {
      ...activeStage(s).binding,
      config: { ...activeStage(s).binding.config, revision: "other" },
    },
  ])
    assert.equal(
      (
        await store.commit({
          ...emptyCommit(s),
          session: replaceStage({ ...s, revision: 1 }, binding, undefined),
        })
      ).ok,
      false,
    );
  assert.equal(
    (
      await store.commit({
        ...emptyCommit(s),
        session: replaceStage({ ...s, revision: 1 }, undefined, {
          ...activeStage(s).capabilities,
          tools: "host_mediated",
        }),
      })
    ).ok,
    false,
  );
  const coordinatesStore = await create();
  unwrap(await coordinatesStore.create(s));
  const coordinates = { nativeRunId: "run-1", nativeRequestId: "request-1" };
  assert.equal(
    (
      await coordinatesStore.commit({
        ...emptyCommit(s),
        session: replaceStage(
          { ...s, revision: 1 },
          { ...activeStage(s).binding, ...coordinates },
          undefined,
        ),
      })
    ).ok,
    false,
  );
  unwrap(await coordinatesStore.accept(acceptance(s)));
  const before = unwrap(await coordinatesStore.session(s.namespace));
  const { record: running } = await dispatchCommand(
    coordinatesStore,
    before,
    "command-1",
    "submitted",
    coordinates,
  );
  const bound = unwrap(await coordinatesStore.session(s.namespace));
  for (const next of [
    {},
    { nativeRunId: "run-1" },
    { nativeRequestId: "request-1" },
    { nativeRunId: "other", nativeRequestId: "request-1" },
    { nativeRunId: "run-1", nativeRequestId: "other" },
  ])
    assert.equal(
      (
        await coordinatesStore.commit({
          ...emptyCommit(bound),
          session: replaceStage(
            { ...bound, revision: bound.revision + 1 },
            { ...activeStage(s).binding, ...next },
            undefined,
          ),
        })
      ).ok,
      false,
    );
  const finished = await terminalCommit(bound, running);
  unwrap(
    await coordinatesStore.commit({
      ...finished,
      session: replaceStage(
        { ...finished.session },
        activeStage(s).binding,
        undefined,
      ),
    }),
  );
  for (const status of ["pending", "unavailable"] as const) {
    const callbacks = await create(),
      seeded = await seedInteraction(callbacks, status);
    for (const patch of [
      { commandId: "other" },
      { nativeRunId: "other" },
      { nativeCallbackId: "other" },
      { request: { question: "different" } },
      { expiresAtMs: 101 },
      { callbackLifetime: "invalid-lifetime" as "generation_bound" },
    ])
      assert.equal(
        (
          await callbacks.commit({
            ...emptyCommit(seeded.session),
            interactions: [{ ...seeded.interaction, ...patch }],
          })
        ).ok,
        false,
      );
    assert.equal(seeded.answer.input.type, "respond");
    if (seeded.answer.input.type !== "respond")
      throw new Error("fixture response required");
    for (const [command, nowMs, code] of [
      [seeded.answer, 101, status === "pending" ? "expired" : "unavailable"],
      [
        {
          ...seeded.answer,
          input: { ...seeded.answer.input, generation: "stale" },
        },
        0,
        status === "pending" ? "stale_binding" : "unavailable",
      ],
    ] as const) {
      const result = await callbacks.accept({
        ...acceptance(seeded.session, command),
        nowMs,
      });
      assert.equal(result.ok, false);
      if (!result.ok) assert.equal(result.error.code, code);
    }
    if (status === "unavailable") {
      assert.equal(
        (
          await callbacks.commit({
            ...emptyCommit(seeded.session),
            interactions: [{ ...seeded.interaction, status: "pending" }],
          })
        ).ok,
        false,
      );
      continue;
    }
    const input = acceptance(seeded.session, seeded.answer);
    const receipt = unwrap(await callbacks.accept(input));
    assert.deepEqual(unwrap(await callbacks.accept(input)), receipt);
    const head = unwrap(await callbacks.session(seeded.session.namespace));
    const second = await callbacks.accept(
      acceptance(head, { ...seeded.answer, commandId: "answer-2" }),
    );
    assert.equal(second.ok, false);
    if (!second.ok) assert.equal(second.error.code, "already_answered");
    assert.equal(
      unwrap(await callbacks.snapshotPage(head.namespace, { limit: 256 }))
        .interactions[0].status,
      "answered",
    );
  }
  // Stable exclusive cursors over several commands and deliveries.
  for (let i = 1; i <= 3; i++) {
    const head = unwrap(await store.session(s.namespace));
    const input = acceptance(head, fixtureCommand(`page-${i}`));
    unwrap(await store.accept(input));
    const next = unwrap(await store.session(s.namespace));
    const acceptedEvent = unwrap(
      await readSnapshot(store, s.namespace),
    ).events.find((e) => e.eventId === input.eventId)!;
    const row: import("../wire.js").Delivery = {
      schemaVersion: 5,
      kind: "delivery",
      namespace: s.namespace,
      operationId: `delivery-${i}`,
      eventId: input.eventId,
      target: "receiver",
      contentHash: deliveryFingerprint(
        acceptedEvent,
        "receiver",
        fixtureLimits,
      ),
      retry: "receiver_idempotent",
      status: "pending",
      attempts: 0,
      nextAttemptAtMs: 0,
    };
    assert.equal(
      (
        await store.commit({
          ...emptyCommit(next),
          deliveries: [{ ...row, contentHash: "0".repeat(64) }],
        })
      ).ok,
      false,
    );
    unwrap(await store.commit({ ...emptyCommit(next), deliveries: [row] }));
    const latest = unwrap(await store.session(s.namespace));
    assert.equal(
      (
        await store.commit({
          ...emptyCommit(latest),
          deliveries: [
            {
              ...row,
              target: "different",
              contentHash: deliveryFingerprint(
                acceptedEvent,
                "different",
                fixtureLimits,
              ),
            },
          ],
        })
      ).ok,
      false,
    );
  }
  const snapshot = unwrap(
    await store.snapshotPage(s.namespace, { limit: 256 }),
  );
  assert.equal(snapshot.events.at(-1)!.sequence, snapshot.cursor);
  const over = await store.snapshotPage(s.namespace, { limit: 1 });
  assert.ok(unwrap(over).next);
  const replay = [];
  let cursor = 0;
  for (let i = 0; i < 3; i++) {
    const page = unwrap(await store.events(s.namespace, cursor, 1));
    assert.equal(page.length, 1);
    replay.push(page[0]);
    cursor = page[0].sequence;
  }
  assert.deepEqual(replay, snapshot.events);
  assert.deepEqual(unwrap(await store.events(s.namespace, cursor, 1)), []);
  const expired = await store.events(s.namespace, cursor + 1, 1);
  assert.equal(expired.ok, false);
  if (!expired.ok) assert.equal(expired.error.code, "cursor_expired");
  for (const kind of ["recovery", "delivery"] as const) {
    let after: string | undefined;
    const keys: string[] = [];
    for (let i = 0; i < 3; i++) {
      const page =
        kind === "recovery"
          ? unwrap(await store.recovery(1, after))
          : unwrap(await store.deliveries(1, 0, after));
      assert.equal(page.items.length, 1);
      const row = page.items[0];
      keys.push(
        row.kind === "delivery" ? row.operationId : row.command.commandId,
      );
      after = page.next;
      assert.equal(after !== undefined, i < 2);
    }
    assert.equal(new Set(keys).size, 3);
  }
}

/** Native terminal evidence and the matching state projection form one commit. */
export async function terminalCommit(
  session: Session,
  record: CommandRecord,
  reconciliation?: VerifiedProviderFact,
): Promise<SessionCommit> {
  if (!record.dispatch)
    throw new Error("fixture requires a dispatched command");
  const dispatch: DispatchAttempt = {
    ...record.dispatch,
    certainty: "submitted",
  };
  const next: CommandRecord = {
    schemaVersion: 5,
    kind: "commandRecord",
    command: record.command,
    receipt: record.receipt,
    state: "terminal",
    dispatch,
    outcome: "completed",
  };
  const bodies: Event["body"][] = [];
  if (record.state === "reconciliation_required" && !reconciliation)
    throw new Error("fixture requires verified reconciliation");
  const providerFacts = [
    reconciliation ??
      (await verifiedReconciliation(session, record, "terminal")),
  ];
  if (providerFacts.length)
    bodies.push({
      type: "reconciled",
      attempt: record.dispatch,
      resolution: "terminal",
    });
  if (record.dispatch.certainty !== "submitted")
    bodies.push({ type: "dispatch", attempt: dispatch });
  bodies.push({ type: "terminal", outcome: "completed" });
  return { ...commandCommit(session, next, bodies), providerFacts };
}
export function commandCommit(
  session: Session,
  record: CommandRecord,
  bodies: readonly Event["body"][],
  nowMs = record.receipt.acceptedAtMs,
): SessionCommit {
  const events = bodies.map(
    (body, i) =>
      ({
        schemaVersion: 5,
        kind: "event",
        namespace: session.namespace,
        eventId: `change-${session.revision}-${record.command.commandId}-${i}`,
        sequence: session.lastSequence + i + 1,
        commandId: record.command.commandId,
        generation: activeStage(session).binding.generation,
        ...(record.dispatch ? { attemptId: record.dispatch.attemptId } : {}),
        body,
      }) as Event,
  );
  return {
    ...emptyCommit(session),
    nowMs,
    session: {
      ...session,
      revision: session.revision + 1,
      lastSequence: session.lastSequence + events.length,
    },
    commands: [record],
    events,
  };
}
export async function dispatchCommand(
  store: SessionStore,
  session: Session,
  id = "command-1",
  certainty: "submitted" | "unknown" = "submitted",
  coordinates: Pick<
    Session["stages"][number]["binding"],
    "nativeRunId" | "nativeRequestId"
  > = {},
  nowMs?: number,
  workingDirectory = ".",
) {
  const record = unwrap(await store.command(session.namespace, id));
  const intent: DispatchAttempt = {
    attemptId: `attempt-${id}`,
    originGeneration: activeStage(session).binding.generation,
    observerGeneration: activeStage(session).binding.generation,
    nativeSessionId: activeStage(session).binding.nativeSessionId,
    ...(activeStage(session).binding.nativeThreadId
      ? { nativeThreadId: activeStage(session).binding.nativeThreadId }
      : {}),
    ...(record.command.input.type === "prompt" &&
    record.command.input.policy === "steer"
      ? { nativeRunId: record.command.input.targetRunId }
      : {}),
    certainty: "intent",
  };
  const preparing: CommandRecord = {
    schemaVersion: 5,
    kind: "commandRecord",
    command: record.command,
    receipt: record.receipt,
    state: "dispatching",
    dispatch: intent,
  };
  unwrap(
    await store.commit(
      commandCommit(
        session,
        preparing,
        [
          { type: "dispatch", attempt: intent },
          { type: "status", state: "dispatching" },
        ],
        nowMs,
      ),
    ),
  );
  const head = unwrap(await store.session(session.namespace));
  const dispatch: DispatchAttempt = {
    ...intent,
    ...coordinates,
    certainty,
    ...(certainty === "unknown" ? { correlationId: `lookup-${id}` } : {}),
  };
  const next: CommandRecord = {
    ...preparing,
    state: certainty === "submitted" ? "running" : "reconciliation_required",
    dispatch,
  };
  const proof = await verifiedReconciliation(
    replaceStage(
      { ...head },
      { ...activeStage(head).binding, ...coordinates },
      undefined,
    ),
    preparing,
    certainty === "submitted" ? "running" : "unknown",
    "completed",
    workingDirectory,
  );
  const batch = commandCommit(
    head,
    next,
    [
      {
        type: "reconciled",
        attempt: intent,
        resolution: certainty === "submitted" ? "running" : "unknown",
      },
      { type: "dispatch", attempt: dispatch },
      {
        type: "status",
        state:
          certainty === "submitted" ? "running" : "reconciliation_required",
      },
    ],
    nowMs,
  );
  unwrap(
    await store.commit({
      ...batch,
      providerFacts: [proof],
      session: replaceStage(
        { ...batch.session },
        { ...activeStage(head).binding, ...coordinates },
        undefined,
      ),
    }),
  );
  return {
    session: unwrap(await store.session(session.namespace)),
    record: next,
  };
}
export function surfaceCommit(
  session: Session,
  surface: SurfaceState,
  interaction: import("../wire.js").Interaction,
): SessionCommit {
  if (surface.status === "deleted" && !surface.messages.at(-1)?.deleteSurface)
    surface = {
      ...surface,
      messages: [
        ...surface.messages,
        { version: "v0.9.1", deleteSurface: { surfaceId: surface.surfaceId } },
      ],
    };
  const event: Event = {
    schemaVersion: 5,
    kind: "event",
    namespace: session.namespace,
    eventId: `surface-${surface.surfaceInstanceId}-${surface.revision}`,
    sequence: session.lastSequence + 1,
    commandId: interaction.commandId,
    attemptId: `attempt-${interaction.commandId}`,
    generation: activeStage(session).binding.generation,
    body: { type: "surface", surface },
  } as Event;
  const interactions =
    surface.status === "deleted" && interaction.status === "pending"
      ? [{ ...interaction, status: "unavailable" as const }]
      : [];
  const events = [
    event,
    ...interactions.map((row) => ({
      ...interactionEvent(session, row),
      sequence: session.lastSequence + 2,
    })),
  ];
  return {
    ...emptyCommit(session),
    session: {
      ...session,
      revision: session.revision + 1,
      lastSequence: session.lastSequence + events.length,
    },
    surfaces: [surface],
    interactions,
    events,
  };
}
export async function seedSurface(
  store: SessionStore,
  session: Session = fixtureSession(),
) {
  const seeded = await seedInteraction(store, "pending", session);
  const template = fixtures.valid.find((v) => v.kind === "surface")!;
  const surface = {
    ...template,
    namespace: seeded.session.namespace,
    generation: seeded.interaction.generation,
    nativeRunId: seeded.interaction.nativeRunId!,
    interactionId: seeded.interaction.interactionId,
    revision: 0,
    status: "active" as const,
  };
  unwrap(
    await store.commit(
      surfaceCommit(seeded.session, surface, seeded.interaction),
    ),
  );
  return {
    ...seeded,
    surface,
    session: unwrap(await store.session(seeded.session.namespace)),
  };
}
async function runSurfaceConformance(store: SessionStore): Promise<void> {
  const seeded = await seedSurface(store),
    { surface } = seeded;
  for (const patch of [
    { revision: 0 },
    { revision: 1 },
    { revision: 2 },
    { revision: 1, sourceComponentId: "different" },
    { revision: 1, interactionId: "different" },
  ])
    assert.equal(
      (
        await store.commit({
          ...emptyCommit(seeded.session),
          surfaces: [{ ...surface, ...patch }],
        })
      ).ok,
      false,
    );
  assert.deepEqual(
    unwrap(await store.session(seeded.session.namespace)),
    seeded.session,
  );
  const malformed = surfaceCommit(
    seeded.session,
    { ...surface, revision: 1 },
    seeded.interaction,
  );
  const malformedEvents = malformed.events.map((e) =>
    e.body.type === "surface"
      ? ({
          ...e,
          body: { ...e.body, surface: { ...e.body.surface, revision: 99 } },
        } as Event)
      : e,
  );
  assert.equal(
    (await store.commit({ ...malformed, events: malformedEvents })).ok,
    false,
  );
  const updated = { ...surface, revision: 1 };
  unwrap(
    await store.commit(
      surfaceCommit(seeded.session, updated, seeded.interaction),
    ),
  );
  const head = unwrap(await store.session(seeded.session.namespace));
  assert.equal(seeded.answer.input.type, "respond");
  if (seeded.answer.input.type !== "respond")
    throw new Error("fixture response required");
  const answer: Command = {
    ...seeded.answer,
    input: {
      ...seeded.answer.input,
      surface: { instanceId: surface.surfaceInstanceId, revision: 0 },
    },
  };
  assert.equal((await store.accept(acceptance(head, answer))).ok, false);
  assert.equal(
    (await store.accept(acceptance(head, seeded.answer))).ok,
    false,
    "surface cannot be omitted",
  );
  unwrap(
    await store.commit(
      surfaceCommit(
        head,
        { ...updated, revision: 2, status: "deleted" },
        seeded.interaction,
      ),
    ),
  );
  const deleted = unwrap(await store.session(head.namespace));
  assert.equal(
    unwrap(await store.surface(head.namespace, surface.surfaceInstanceId))
      .status,
    "deleted",
  );
  assert.equal(
    unwrap(await store.snapshotPage(head.namespace, { limit: 256 }))
      .interactions[0].status,
    "unavailable",
  );
  assert.equal(
    (
      await store.accept(
        acceptance(deleted, {
          ...seeded.answer,
          input: {
            ...seeded.answer.input,
            surface: { instanceId: surface.surfaceInstanceId, revision: 2 },
          },
        }),
      )
    ).ok,
    false,
  );
  assert.equal(
    (
      await store.commit({
        ...emptyCommit(deleted),
        surfaces: [{ ...updated, revision: 3 }],
      })
    ).ok,
    false,
    "tombstone cannot resurrect",
  );
  assert.equal(
    (
      await store.surface(
        { ...head.namespace, tenantId: "other" },
        surface.surfaceInstanceId,
      )
    ).ok,
    false,
  );
}

/** Stable projection for deterministic store fixtures; no callback or execution authority. */
export function interactionEvent(
  session: Session,
  row: import("../wire.js").Interaction,
): import("../wire.js").Event {
  return {
    schemaVersion: 5,
    kind: "event",
    namespace: session.namespace,
    eventId: `interaction-${row.interactionId}-${row.status}`,
    sequence: session.lastSequence + 1,
    commandId: row.commandId,
    attemptId: `attempt-${row.commandId}`,
    generation: row.generation,
    body: {
      type: "interaction",
      interactionId: row.interactionId,
      ...(row.status === "pending"
        ? {
            status: "pending",
            request: row.request,
            expiresAtMs: row.expiresAtMs,
            callbackLifetime: row.callbackLifetime,
          }
        : row.status === "answered"
          ? { status: "answered", responseCommandId: row.responseCommandId! }
          : { status: row.status }),
    },
  } as Event;
}

/** Every store consumer proves callback identity separately from its parent prompt. */
async function runCallbackConformance(store: SessionStore): Promise<void> {
  const seeded = await seedInteraction(store);
  const observation: Extract<
    import("../ports.js").ProviderObservation,
    { type: "interaction" }
  > = {
    type: "interaction",
    attemptId: "attempt-command-1",
    binding: {
      ...activeStage(seeded.session).binding,
      nativeRunId: seeded.interaction.nativeRunId,
    },
    commandId: seeded.interaction.commandId,
    interaction: {
      category: "question",
      interactionId: "question-2",
      nativeCallbackId: "callback-2",
      expiresAtMs: 100,
      callbackLifetime: "generation_bound",
      request: { question: "Second independent question" },
    },
  };
  const row: import("../wire.js").Interaction = {
    schemaVersion: 5,
    kind: "interaction",
    namespace: seeded.session.namespace,
    commandId: observation.commandId,
    generation: observation.binding.generation,
    nativeRunId: observation.binding.nativeRunId,
    status: "pending",
    ...observation.interaction,
  };
  const batch = {
    ...emptyCommit(seeded.session),
    session: {
      ...seeded.session,
      revision: seeded.session.revision + 1,
      lastSequence: seeded.session.lastSequence + 1,
    },
    interactions: [row],
    events: [interactionEvent(seeded.session, row)],
  };
  assert.equal((await store.commit({ ...batch, interactions: [] })).ok, false);
  assert.equal(
    (
      await store.commit({
        ...emptyCommit(seeded.session),
        interactions: [row],
      })
    ).ok,
    false,
  );
  for (const request of [
    undefined,
    { question: "different from the pending record" },
  ]) {
    const event = batch.events[0];
    assert.equal(
      (
        await store.commit({
          ...batch,
          events: [
            {
              ...event,
              body: {
                type: "interaction",
                interactionId: row.interactionId,
                status: "pending",
                expiresAtMs: row.expiresAtMs,
                callbackLifetime: row.callbackLifetime,
                ...(request === undefined ? {} : { request }),
              } as Event["body"],
            } as Event,
          ],
        })
      ).ok,
      false,
    );
    const unchanged = unwrap(
      await store.snapshotPage(seeded.session.namespace, { limit: 256 }),
    );
    assert.equal(unchanged.interactions.length, 1);
    assert.equal(unchanged.cursor, seeded.session.lastSequence);
  }
  unwrap(await store.commit(batch));
  let head = unwrap(await store.session(seeded.session.namespace));
  const alias = { ...row, interactionId: "callback-alias" };
  assert.equal(
    (
      await store.commit({
        ...emptyCommit(head),
        session: {
          ...head,
          revision: head.revision + 1,
          lastSequence: head.lastSequence + 1,
        },
        interactions: [alias],
        events: [interactionEvent(head, alias)],
      })
    ).ok,
    false,
  );
  for (const interaction of [row, seeded.interaction]) {
    head = unwrap(await store.session(head.namespace));
    unwrap(
      await store.accept(
        acceptance(head, {
          ...fixtureCommand(`answer-${interaction.interactionId}`),
          input: {
            type: "respond",
            interactionId: interaction.interactionId,
            generation: interaction.generation,
            nativeRunId: interaction.nativeRunId,
            answer: { choice: "a" },
          },
        }),
      ),
    );
  }
  const snapshot = unwrap(
    await store.snapshotPage(head.namespace, { limit: 256 }),
  );
  assert.equal(snapshot.interactions.length, 2);
  assert.ok(snapshot.interactions.every((row) => row.status === "answered"));
  assert.deepEqual(
    snapshot.interactions.find(
      (row) => row.interactionId === observation.interaction.interactionId,
    )?.request,
    observation.interaction.request,
  );
}

/** Deterministic dispatch metadata for adapter fixtures, never real durability evidence. */
export function fixtureAttempt(
  binding: Session["stages"][number]["binding"],
  command: Command,
): DispatchAttempt {
  return {
    attemptId: `attempt-${command.commandId}`,
    originGeneration: binding.generation,
    observerGeneration: binding.generation,
    nativeSessionId: binding.nativeSessionId,
    certainty: "intent",
  };
}
export function fixtureDispatchedRecord(
  binding: Session["stages"][number]["binding"],
  command: Command,
  namespace = fixtureSession().namespace,
): CommandRecord {
  return {
    schemaVersion: 5,
    kind: "commandRecord",
    command,
    receipt: {
      schemaVersion: 5,
      kind: "receipt",
      stageId: binding.generation,
      namespace,
      commandId: command.commandId,
      contentHash: fingerprint(command, fixtureLimits),
      acceptedAtMs: 0,
      retryUntilMs: command.expiresAtMs,
      receiptUntilMs: command.expiresAtMs,
      acceptedRevision: 1,
    },
    state: "reconciliation_required",
    dispatch: {
      ...fixtureAttempt(binding, command),
      certainty: "unknown",
      correlationId: command.commandId,
    },
  };
}
export function fixtureProviderSession(
  binding: Session["stages"][number]["binding"],
  configuration: import("../ports.js").ProviderConfiguration,
): Session {
  return replaceStage(
    { ...fixtureSession(), namespace: configuration.namespace },
    binding,
    {
      ...activeStage(fixtureSession()).capabilities,
      tools:
        configuration.permissions === "host_mediated"
          ? "host_mediated"
          : "disabled",
      continuation: "across_processes",
    },
  );
}
