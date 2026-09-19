import assert from "node:assert/strict";
import test from "node:test";
import { VerifiedProviderSession } from "../../../packages/ai-contract/dist/session.js";
import {
  MemorySessionStore,
  acceptance,
  commandCommit,
  dispatchCommand,
  verifiedReconciliation,
  unwrap,
} from "../../../packages/ai-contract/dist/testing/index.js";
import { createTestAdapter } from "../../../packages/ai-adapters/codex/dist/testing.js";
import {
  NativeNotSubmittedError,
  classifyNativeRejection,
  nativeRuntime,
} from "../../../packages/ai-adapters/codex/dist/runtime.js";
import { budget, conversation, nativeFixture, prompt } from "./helpers.mjs";

async function persistIntent(store, session, command, attempt) {
  const accepted = unwrap(
    await store.command(session.namespace, command.commandId),
  );
  const record = {
    schemaVersion: 3,
    kind: "commandRecord",
    command: accepted.command,
    receipt: accepted.receipt,
    state: "dispatching",
    dispatch: structuredClone(attempt),
  };
  unwrap(
    await store.commit({
      ...commandCommit(session, record, [
        { type: "dispatch", attempt: record.dispatch },
        { type: "status", state: "dispatching" },
      ]),
      nowMs: 1,
    }),
  );
  return {
    session: unwrap(await store.session(session.namespace)),
    record,
  };
}

async function persistDispatch(
  store,
  persisted,
  certainty,
  coordinates = {},
  workingDirectory = ".",
) {
  const dispatch = {
    ...persisted.record.dispatch,
    ...coordinates,
    certainty,
    ...(certainty === "unknown"
      ? { correlationId: `lookup-${persisted.record.command.commandId}` }
      : {}),
  };
  const record = {
    ...persisted.record,
    state: certainty === "submitted" ? "running" : "reconciliation_required",
    dispatch,
  };
  const resolution = certainty === "submitted" ? "running" : "unknown";
  const proof = await verifiedReconciliation(
    {
      ...persisted.session,
      binding: { ...persisted.session.binding, ...coordinates },
    },
    persisted.record,
    resolution,
    "completed",
    workingDirectory,
  );
  const batch = commandCommit(persisted.session, record, [
    { type: "reconciled", attempt: persisted.record.dispatch, resolution },
    { type: "dispatch", attempt: dispatch },
    { type: "status", state: record.state },
  ]);
  unwrap(
    await store.commit({
      ...batch,
      providerFacts: [proof],
      session: {
        ...batch.session,
        binding: { ...persisted.session.binding, ...coordinates },
      },
    }),
  );
  return {
    session: unwrap(await store.session(persisted.session.namespace)),
    record,
  };
}

test("runtime recognizes only the fixed steer not-submitted rejection", () => {
  const exact = classifyNativeRejection("turn/steer", {
    code: -32600,
    message: "no active turn to steer",
  });
  assert.ok(exact instanceof NativeNotSubmittedError);
  assert.equal(exact.message, "native request was not submitted");

  for (const [method, error] of [
    ["turn/start", { code: -32600, message: "no active turn to steer" }],
    ["turn/steer", { code: -32601, message: "no active turn to steer" }],
    ["turn/steer", { code: -32600, message: "internal error" }],
    [
      "turn/steer",
      { code: -32600, message: "no active turn to steer", data: null },
    ],
    [
      "turn/steer",
      { code: -32600, message: "no active turn to steer", extra: true },
    ],
  ])
    assert.equal(
      classifyNativeRejection(method, error),
      undefined,
      `${method}: ${JSON.stringify(error)}`,
    );
});

test(
  "fixed app-server: an explicitly rejected steer is not an unknown submission",
  { timeout: 30000 },
  async (t) => {
    const fixture = await nativeFixture(t);
    let releaseCompleted, rejected;
    let completionHeld = false;
    const methods = [];
    const completed = new Promise((resolve) => {
      releaseCompleted = resolve;
    });
    const port = createTestAdapter(fixture.options, (spec) => {
      const native = nativeRuntime(spec);
      return {
        stopped: native.stopped,
        async request(...args) {
          methods.push(args[0]);
          try {
            return await native.request(...args);
          } catch (error) {
            if (error instanceof NativeNotSubmittedError) rejected = error;
            throw error;
          }
        },
        notify: (...args) => native.notify(...args),
        reply: (...args) => native.reply(...args),
        reject: (...args) => native.reject(...args),
        close: (...args) => native.close(...args),
        listen(listener) {
          native.listen((message) => {
            if (message.method === "turn/completed" && !completionHeld) {
              completionHeld = true;
              releaseCompleted(() => listener(message));
              return;
            }
            listener(message);
          });
        },
      };
    });
    fixture.ports.push(port);
    const admitted = unwrap(
      await VerifiedProviderSession.open(port, fixture.configuration, budget()),
    );
    const session = {
      schemaVersion: 3,
      kind: "session",
      namespace: fixture.configuration.namespace,
      revision: 0,
      lastSequence: 0,
      status: "active",
      binding: admitted.binding,
      capabilities: admitted.capabilities,
    };
    const store = new MemorySessionStore();
    unwrap(await store.create(session));
    const first = prompt(admitted.binding, "first-before-steer-race");
    unwrap(await store.accept(acceptance(session, first.command)));
    const firstAccepted = unwrap(await store.session(session.namespace));
    const firstIntent = await persistIntent(
      store,
      firstAccepted,
      first.command,
      first.attempt,
    );
    assert.equal(firstIntent.record.state, "dispatching");
    assert.deepEqual(firstIntent.record.dispatch, first.attempt);
    const started = await port.dispatch(
      admitted.binding,
      first.command,
      firstIntent.record.dispatch,
      budget(),
    );
    assert.equal(started.certainty, "submitted");
    const running = await persistDispatch(
      store,
      firstIntent,
      "submitted",
      {
        nativeRunId: started.binding.nativeRunId,
        nativeRequestId: started.binding.nativeRequestId,
      },
      fixture.configuration.workingDirectory,
    );
    const deliverCompleted = await completed;

    const steer = prompt(admitted.binding, "rejected-steer");
    steer.command.input = {
      type: "prompt",
      policy: "steer",
      text: "fixture steer",
      targetRunId: started.binding.nativeRunId,
    };
    steer.attempt.nativeRunId = started.binding.nativeRunId;
    unwrap(await store.accept(acceptance(running.session, steer.command)));
    const steerAccepted = unwrap(await store.session(session.namespace));
    const steerIntent = await persistIntent(
      store,
      steerAccepted,
      steer.command,
      steer.attempt,
    );
    assert.equal(steerIntent.record.state, "dispatching");
    assert.deepEqual(steerIntent.record.dispatch, steer.attempt);
    const submission = await port.dispatch(
      admitted.binding,
      steer.command,
      steerIntent.record.dispatch,
      budget(),
    );
    deliverCompleted();

    assert.ok(rejected instanceof NativeNotSubmittedError);
    assert.equal(submission.certainty, "not_sent");
    assert.deepEqual(Object.keys(submission.error).sort(), ["code", "retry"]);
    assert.deepEqual(
      await port.dispatch(
        admitted.binding,
        steer.command,
        steerIntent.record.dispatch,
        budget(),
      ),
      submission,
      "the rejected attempt remains definitively not sent",
    );
    assert.equal(
      methods.filter((method) => method === "turn/steer").length,
      1,
      "same-attempt retry must not reach the native runtime",
    );

    const durable = await persistDispatch(
      store,
      steerIntent,
      "unknown",
      {
        nativeRunId: started.binding.nativeRunId,
      },
      fixture.configuration.workingDirectory,
    );
    assert.equal(durable.record.state, "reconciliation_required");
    assert.equal(
      durable.record.dispatch.attemptId,
      steerIntent.record.dispatch.attemptId,
    );
    const proof = unwrap(
      await admitted.reconcile(durable.session, durable.record, budget()),
    );
    assert.equal(proof.commandId, steer.command.commandId);
    assert.deepEqual(proof.observation, {
      commandId: steer.command.commandId,
      attemptId: steer.attempt.attemptId,
      binding: {
        ...admitted.binding,
        nativeRunId: started.binding.nativeRunId,
      },
      status: "not_submitted",
    });
    for (const [changedSession, changedRecord] of [
      [
        durable.session,
        {
          ...durable.record,
          command: {
            ...durable.record.command,
            input: { ...durable.record.command.input, text: "changed" },
          },
        },
      ],
      [
        durable.session,
        {
          ...durable.record,
          dispatch: {
            ...durable.record.dispatch,
            attemptId: "different-attempt",
          },
        },
      ],
      [
        durable.session,
        {
          ...durable.record,
          dispatch: {
            ...durable.record.dispatch,
            originGeneration: "different-origin",
          },
        },
      ],
      [
        durable.session,
        {
          ...durable.record,
          dispatch: {
            ...durable.record.dispatch,
            certainty: "submitted",
          },
        },
      ],
      [
        durable.session,
        {
          ...durable.record,
          dispatch: {
            ...durable.record.dispatch,
            nativeRunId: "different-run",
          },
        },
      ],
      [
        durable.session,
        {
          ...durable.record,
          dispatch: {
            ...durable.record.dispatch,
            nativeRequestId: "different-request",
          },
        },
      ],
      [
        durable.session,
        {
          ...durable.record,
          dispatch: {
            ...durable.record.dispatch,
            nativeSessionId: "different-session",
          },
        },
      ],
      [
        durable.session,
        {
          ...durable.record,
          dispatch: {
            ...durable.record.dispatch,
            nativeThreadId: "different-thread",
          },
        },
      ],
      [
        durable.session,
        {
          ...durable.record,
          dispatch: {
            ...durable.record.dispatch,
            observerGeneration: "different-observer",
          },
        },
      ],
      [
        {
          ...durable.session,
          binding: {
            ...durable.session.binding,
            nativeThreadId: "different-thread",
          },
        },
        durable.record,
      ],
    ])
      assert.equal(
        (await admitted.reconcile(changedSession, changedRecord, budget())).ok,
        false,
      );
    const failure = { code: "stale_binding", retry: "never" };
    const invalidated = {
      schemaVersion: 3,
      kind: "commandRecord",
      command: durable.record.command,
      receipt: durable.record.receipt,
      state: "invalidated",
      failure,
    };
    const invalidation = commandCommit(durable.session, invalidated, [
      {
        type: "reconciled",
        attempt: durable.record.dispatch,
        resolution: "not_submitted",
      },
      { type: "invalidated", failure },
    ]);
    invalidation.events[0].attemptId = durable.record.dispatch.attemptId;
    unwrap(
      await store.commit({
        ...invalidation,
        nowMs: 1,
        providerFacts: [proof],
      }),
    );
    const settledSteer = unwrap(
      await store.command(session.namespace, steer.command.commandId),
    );
    assert.equal(settledSteer.state, "invalidated");
    assert.deepEqual(settledSteer.failure, failure);

    const next = prompt(admitted.binding, "ordinary-after-rejection");
    const nextSubmission = await port.dispatch(
      admitted.binding,
      next.command,
      next.attempt,
      budget(),
    );
    assert.equal(nextSubmission.certainty, "submitted");
    let nextTerminal;
    for await (const observation of port.observe(admitted.binding, budget())) {
      if (
        observation.type === "event" &&
        observation.commandId === next.command.commandId &&
        observation.body.type === "terminal"
      ) {
        nextTerminal = observation;
        break;
      }
    }
    assert.equal(nextTerminal?.body.outcome, "completed");
    assert.equal(fixture.requests.length, 2);
  },
);

test(
  "fixed app-server: a cold port does not invent not-submitted evidence for missing history",
  { timeout: 30000 },
  async (t) => {
    const fixture = await nativeFixture(t);
    const firstPort = fixture.make();
    const admitted = unwrap(
      await VerifiedProviderSession.open(
        firstPort,
        fixture.configuration,
        budget(),
      ),
    );
    const persisted = await conversation(firstPort, admitted, "persisted");
    const previous = {
      schemaVersion: 3,
      kind: "session",
      namespace: fixture.configuration.namespace,
      revision: 0,
      lastSequence: 0,
      status: "active",
      binding: persisted.submission.binding,
      capabilities: admitted.capabilities,
    };
    fixture.lineage.set(admitted.binding.nativeThreadId, {
      nativeSessionId: admitted.binding.nativeSessionId,
      nativeThreadId: admitted.binding.nativeThreadId,
    });
    unwrap(await firstPort.close(budget()));

    const recoveryPort = fixture.make();
    const restored = unwrap(
      await VerifiedProviderSession.restore(
        recoveryPort,
        previous,
        fixture.configuration,
        budget(),
      ),
    );
    const current = {
      ...previous,
      binding: restored.binding,
      capabilities: restored.capabilities,
    };
    const store = new MemorySessionStore();
    unwrap(await store.create(current));
    const missing = prompt(restored.binding, "cold-missing-history");
    unwrap(await store.accept(acceptance(current, missing.command)));
    const accepted = unwrap(await store.session(current.namespace));
    const durable = await dispatchCommand(
      store,
      accepted,
      missing.command.commandId,
      "unknown",
      {},
      undefined,
      fixture.configuration.workingDirectory,
    );
    const proof = unwrap(
      await restored.reconcile(durable.session, durable.record, budget()),
    );
    assert.equal(proof.observation.attemptId, missing.attempt.attemptId);
    assert.equal(proof.observation.status, "unknown");
  },
);

test(
  "a contradictory rejection cannot erase an observed submission fact",
  { timeout: 30000 },
  async (t) => {
    const fixture = await nativeFixture(t);
    let adapterListener, releaseCompleted;
    let completionHeld = false;
    const completed = new Promise((resolve) => {
      releaseCompleted = resolve;
    });
    const methods = [];
    const port = createTestAdapter(fixture.options, (spec) => {
      const native = nativeRuntime(spec);
      return {
        stopped: native.stopped,
        async request(method, params, requestBudget) {
          methods.push(method);
          const response = native.request(method, params, requestBudget);
          if (method === "turn/steer")
            adapterListener({
              method: "item/started",
              params: {
                threadId: params.threadId,
                turnId: params.expectedTurnId,
                item: {
                  id: "contradictory-user-message",
                  type: "userMessage",
                  clientId: params.clientUserMessageId,
                },
              },
            });
          return await response;
        },
        notify: (...args) => native.notify(...args),
        reply: (...args) => native.reply(...args),
        reject: (...args) => native.reject(...args),
        close: (...args) => native.close(...args),
        listen(listener) {
          adapterListener = listener;
          native.listen((message) => {
            if (message.method === "turn/completed" && !completionHeld) {
              completionHeld = true;
              releaseCompleted(() => listener(message));
              return;
            }
            listener(message);
          });
        },
      };
    });
    fixture.ports.push(port);
    const admitted = unwrap(
      await VerifiedProviderSession.open(port, fixture.configuration, budget()),
    );
    const first = prompt(admitted.binding, "contradiction-first");
    const started = await port.dispatch(
      admitted.binding,
      first.command,
      first.attempt,
      budget(),
    );
    assert.equal(started.certainty, "submitted");
    const deliverCompleted = await completed;
    const steer = prompt(admitted.binding, "contradiction-steer");
    steer.command.input = {
      type: "prompt",
      policy: "steer",
      text: "fixture steer",
      targetRunId: started.binding.nativeRunId,
    };
    steer.attempt.nativeRunId = started.binding.nativeRunId;
    const submission = await port.dispatch(
      admitted.binding,
      steer.command,
      steer.attempt,
      budget(),
    );
    deliverCompleted();
    assert.equal(submission.certainty, "acknowledged");
    assert.deepEqual(
      await port.dispatch(
        admitted.binding,
        steer.command,
        steer.attempt,
        budget(),
      ),
      submission,
    );
    assert.equal(methods.filter((method) => method === "turn/steer").length, 1);
  },
);
