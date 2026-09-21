import { activeStage } from "../../packages/ai-contract/dist/index.js";
import assert from "node:assert/strict";
import test from "node:test";
import { VerifiedProviderSession } from "../../packages/ai-contract/dist/session.js";
import {
  MemorySessionStore,
  FakeHost,
  fixtureSession,
  fixtureCommand,
  fixtureCaller,
  acceptance,
  commandCommit,
  dispatchCommand,
  terminalCommit,
  verifiedReconciliation,
  emptyCommit,
  unwrap,
} from "../../packages/ai-contract/dist/testing/index.js";

async function accept(store, command) {
  const head = unwrap(await store.session(fixtureSession().namespace));
  unwrap(await store.accept(acceptance(head, command)));
  return unwrap(await store.session(head.namespace));
}
function intent(session, record, coordinates = {}) {
  const dispatch = {
    attemptId: `attempt-${record.command.commandId}`,
    originGeneration: activeStage(session).binding.generation,
    observerGeneration: activeStage(session).binding.generation,
    nativeSessionId: activeStage(session).binding.nativeSessionId,
    ...(activeStage(session).binding.nativeThreadId
      ? { nativeThreadId: activeStage(session).binding.nativeThreadId }
      : {}),
    certainty: "intent",
    ...coordinates,
  };
  return commandCommit(session, { ...record, state: "dispatching", dispatch }, [
    { type: "dispatch", attempt: dispatch },
    { type: "status", state: "dispatching" },
  ]);
}

test("dispatch serializes ordinary turns including unknown submissions", async () => {
  for (const certainty of ["submitted", "unknown"]) {
    const store = new MemorySessionStore(),
      session = fixtureSession();
    unwrap(await store.create(session));
    const first = await accept(store, fixtureCommand());
    await dispatchCommand(store, first, "command-1", certainty);
    const head = await accept(store, fixtureCommand("next"));
    const next = unwrap(await store.command(head.namespace, "next"));
    assert.equal((await store.commit(intent(head, next))).ok, false);
    assert.equal(
      unwrap(await store.command(head.namespace, "next")).state,
      "accepted",
    );
  }
});

test("thread identity survives dispatch and restore, and cannot change under the same root", async () => {
  const store = new MemorySessionStore(),
    session = fixtureSession();
  activeStage(session).binding.nativeThreadId = "thread-1";
  unwrap(await store.create(session));
  const head = await accept(store, fixtureCommand());
  const record = unwrap(await store.command(head.namespace, "command-1"));
  assert.equal(
    (await store.commit(intent(head, record, { nativeThreadId: "sibling" })))
      .ok,
    false,
  );
  unwrap(await store.commit(intent(head, record)));
  const configuration = {
    namespace: session.namespace,
    provider: activeStage(session).binding.provider,
    config: activeStage(session).binding.config,

    workingDirectory: ".",
    permissions: "tools_disabled",
  };
  for (const thread of ["thread-1", "sibling", undefined]) {
    const binding = { ...activeStage(session).binding, generation: "restored" };
    if (thread) binding.nativeThreadId = thread;
    else delete binding.nativeThreadId;
    const result = await VerifiedProviderSession.restore(
      {
        resume: async () => ({
          ok: true,
          value: {
            binding,
            capabilities: {
              ...activeStage(session).capabilities,
              continuation: "across_processes",
            },
          },
        }),
        close: async () => ({ ok: true, value: { processStopped: true } }),
      },
      session,
      configuration,
      { timeoutMs: 1000, signal: new AbortController().signal },
    );
    assert.equal(result.ok, thread === "thread-1");
  }
});

test("an existing dispatch cannot acquire a thread outside its session binding", async () => {
  const store = new MemorySessionStore(),
    session = fixtureSession();
  unwrap(await store.create(session));
  let head = await accept(store, fixtureCommand());
  const record = unwrap(await store.command(head.namespace, "command-1"));
  unwrap(await store.commit(intent(head, record)));
  head = unwrap(await store.session(head.namespace));
  const dispatched = unwrap(await store.command(head.namespace, "command-1"));
  const changed = {
    ...dispatched,
    dispatch: { ...dispatched.dispatch, nativeThreadId: "foreign-thread" },
  };
  assert.equal(
    (
      await store.commit(
        commandCommit(head, changed, [
          { type: "dispatch", attempt: changed.dispatch },
        ]),
      )
    ).ok,
    false,
  );
});

test("steer observations use their own request; unresolved steer prevents clearing its turn", async () => {
  const store = new MemorySessionStore(),
    session = fixtureSession();
  activeStage(session).capabilities.steer = "supported";
  unwrap(await store.create(session));
  let head = await accept(store, fixtureCommand());
  const started = await dispatchCommand(store, head, "command-1", "submitted", {
    nativeRunId: "turn-1",
    nativeRequestId: "start",
  });
  head = await accept(store, {
    ...fixtureCommand("steer"),
    input: {
      type: "prompt",
      policy: "steer",
      targetRunId: "turn-1",
      text: "redirect",
    },
  });
  const record = unwrap(await store.command(head.namespace, "steer"));
  unwrap(
    await store.commit(
      intent(head, record, {
        nativeRunId: "turn-1",
        nativeRequestId: "steer-request",
      }),
    ),
  );
  head = unwrap(await store.session(head.namespace));
  const dispatching = unwrap(await store.command(head.namespace, "steer"));
  const running = {
    ...dispatching,
    state: "running",
    dispatch: { ...dispatching.dispatch, certainty: "submitted" },
  };
  unwrap(
    await store.commit({
      ...commandCommit(head, running, [
        {
          type: "reconciled",
          attempt: dispatching.dispatch,
          resolution: "running",
        },
        { type: "dispatch", attempt: running.dispatch },
        { type: "status", state: "running" },
      ]),
      providerFacts: [
        await verifiedReconciliation(head, dispatching, "running"),
      ],
    }),
  );
  const host = new FakeHost(store);
  const observation = {
    type: "delta",
    binding: { ...activeStage(head).binding, nativeRequestId: "steer-request" },
    commandId: "steer",
    attemptId: running.dispatch.attemptId,
    messageId: "item-1",
    text: "delta",
  };
  unwrap(
    await host.publishDelta(
      fixtureCaller,
      session.namespace.sessionId,
      observation,
    ),
  );
  assert.equal(
    (
      await host.publishDelta(fixtureCaller, session.namespace.sessionId, {
        ...observation,
        binding: { ...observation.binding, nativeRequestId: "wrong" },
      })
    ).ok,
    false,
  );
  head = unwrap(await store.session(head.namespace));
  unwrap(await store.commit(await terminalCommit(head, started.record)));
  head = unwrap(await store.session(head.namespace));
  const batch = emptyCommit(head);
  activeStage(batch.session).binding = { ...activeStage(session).binding };
  assert.equal(
    (await store.commit(batch)).ok,
    false,
    "steer still owns the active turn",
  );
  const acknowledged = {
    ...running,
    state: "acknowledged",
    acknowledgement: { type: "steer" },
  };
  unwrap(
    await store.commit({
      ...commandCommit(head, acknowledged, [
        {
          type: "reconciled",
          attempt: running.dispatch,
          resolution: "acknowledged",
        },
        { type: "acknowledged", acknowledgement: acknowledged.acknowledgement },
      ]),
      providerFacts: [
        await verifiedReconciliation(
          head,
          running,
          "acknowledged",
          "completed",
          ".",
          { type: "steer" },
        ),
      ],
    }),
  );
  head = unwrap(await store.session(head.namespace));
  const clear = emptyCommit(head);
  activeStage(clear.session).binding = { ...activeStage(session).binding };
  unwrap(await store.commit(clear));
});

test("steer requires an active matching turn at actual dispatch", async () => {
  const store = new MemorySessionStore(),
    session = fixtureSession();
  activeStage(session).capabilities.steer = "supported";
  unwrap(await store.create(session));
  const command = {
    ...fixtureCommand("steer"),
    input: {
      type: "prompt",
      policy: "steer",
      targetRunId: "gone",
      text: "redirect",
    },
  };
  const head = await accept(store, command);
  const record = unwrap(await store.command(head.namespace, command.commandId));
  assert.equal(
    (await store.commit(intent(head, record, { nativeRunId: "gone" }))).ok,
    false,
  );
});
