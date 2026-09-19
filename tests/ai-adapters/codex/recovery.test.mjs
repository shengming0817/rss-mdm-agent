import assert from "node:assert/strict";
import test from "node:test";
import { VerifiedProviderSession } from "../../../packages/ai-contract/dist/session.js";
import {
  MemorySessionStore,
  acceptance,
  commandCommit,
  dispatchCommand,
  unwrap,
} from "../../../packages/ai-contract/dist/testing/index.js";
import { createTestAdapter } from "../../../packages/ai-adapters/codex/dist/testing.js";
import { nativeRuntime } from "../../../packages/ai-adapters/codex/dist/runtime.js";
import { budget, nativeFixture, prompt } from "./helpers.mjs";

function instrumentRuntime(record) {
  return (spec) => {
    const connection = nativeRuntime(spec);
    const request = connection.request.bind(connection);
    record.connection = connection;
    record.pid = connection.child?.pid;
    connection.request = (method, params, requestBudget) => {
      record.methods.push(method);
      return request(method, params, requestBudget);
    };
    return connection;
  };
}

async function hardKill(record) {
  assert.ok(Number.isSafeInteger(record.pid) && record.pid > 0);
  if (process.platform === "win32") record.connection.child.kill("SIGKILL");
  else process.kill(-record.pid, "SIGKILL");
  await record.connection.stopped;
}

test(
  "fixed app-server: a killed active turn cold-reconciles the original attempt without replay",
  { timeout: 60000 },
  async (t) => {
    let entered;
    const activeRequest = new Promise((resolve) => {
      entered = resolve;
    });
    const first = await nativeFixture(t, {
      handleModel: async () => {
        entered();
        await new Promise(() => {});
      },
    });
    const firstRuntime = { methods: [] };
    const firstPort = createTestAdapter(
      first.options,
      instrumentRuntime(firstRuntime),
    );
    first.ports.push(firstPort);
    const admitted = unwrap(
      await VerifiedProviderSession.open(
        firstPort,
        first.configuration,
        budget(),
      ),
    );

    const initial = {
      schemaVersion: 2,
      kind: "session",
      namespace: first.configuration.namespace,
      revision: 0,
      lastSequence: 0,
      status: "active",
      binding: admitted.binding,
      capabilities: admitted.capabilities,
    };
    const store = new MemorySessionStore();
    unwrap(await store.create(initial));
    const pending = prompt(admitted.binding, "killed-active-turn");
    unwrap(await store.accept(acceptance(initial, pending.command)));
    const accepted = unwrap(await store.session(initial.namespace));
    const submission = await firstPort.submit(
      admitted.binding,
      pending.command,
      pending.attempt,
      budget(),
    );
    assert.equal(submission.certainty, "submitted");
    const durable = await dispatchCommand(
      store,
      accepted,
      pending.command.commandId,
      "submitted",
      {
        nativeRunId: submission.binding.nativeRunId,
        nativeRequestId: submission.binding.nativeRequestId,
      },
    );
    assert.equal(durable.record.dispatch.attemptId, pending.attempt.attemptId);
    assert.equal(
      durable.record.dispatch.nativeThreadId,
      admitted.binding.nativeThreadId,
    );

    await activeRequest;
    await hardKill(firstRuntime);
    assert.equal(first.requests.length, 1);

    first.lineage.set(admitted.binding.nativeThreadId, {
      nativeSessionId: admitted.binding.nativeSessionId,
      nativeThreadId: admitted.binding.nativeThreadId,
    });
    const recoveryRuntime = { methods: [] };
    const recoveryPort = createTestAdapter(
      first.options,
      instrumentRuntime(recoveryRuntime),
    );
    first.ports.push(recoveryPort);
    const restoreResult = await VerifiedProviderSession.restore(
      recoveryPort,
      durable.session,
      first.configuration,
      budget(),
    );
    assert.equal(
      restoreResult.ok,
      true,
      JSON.stringify({ restoreResult, methods: recoveryRuntime.methods }),
    );
    const restored = unwrap(restoreResult);
    assert.notEqual(
      restored.binding.generation,
      durable.session.binding.generation,
    );
    assert.equal(
      restored.binding.nativeSessionId,
      durable.session.binding.nativeSessionId,
    );
    assert.equal(
      restored.binding.nativeThreadId,
      durable.session.binding.nativeThreadId,
    );

    const rebound = unwrap(
      await store.rebind({
        namespace: durable.session.namespace,
        expectedRevision: durable.session.revision,
        expectedGeneration: durable.session.binding.generation,
        restored,
        eventId: "cold-recovery-rebind",
      }),
    );
    const record = unwrap(
      await store.command(rebound.namespace, pending.command.commandId),
    );
    assert.equal(record.state, "reconciliation_required");
    assert.equal(record.dispatch.attemptId, pending.attempt.attemptId);
    assert.equal(record.dispatch.nativeRunId, submission.binding.nativeRunId);
    assert.equal(
      record.dispatch.nativeRequestId,
      submission.binding.nativeRequestId,
    );

    const history = unwrap(
      await recoveryPort.readHistory(restored.binding, budget()),
    );
    const matching = history.filter((turn) =>
      turn.items.some(
        (item) =>
          item.type === "userMessage" &&
          item.clientId === pending.attempt.attemptId,
      ),
    );
    assert.equal(matching.length, 1);
    assert.equal(matching[0].id, submission.binding.nativeRunId);
    assert.equal(matching[0].status, "interrupted");

    const proof = unwrap(await restored.reconcile(rebound, record, budget()));
    assert.equal(proof.commandId, pending.command.commandId);
    assert.equal(proof.observation.attemptId, pending.attempt.attemptId);
    assert.equal(
      proof.observation.binding.nativeRunId,
      submission.binding.nativeRunId,
    );
    assert.equal(
      proof.observation.binding.nativeRequestId,
      submission.binding.nativeRequestId,
    );
    assert.equal(
      proof.observation.binding.nativeSessionId,
      admitted.binding.nativeSessionId,
    );
    assert.equal(
      proof.observation.binding.nativeThreadId,
      admitted.binding.nativeThreadId,
    );
    assert.equal(proof.observation.status, "terminal");
    assert.equal(proof.observation.outcome, "cancelled");
    const terminal = {
      ...record,
      state: "terminal",
      dispatch: { ...record.dispatch, certainty: "submitted" },
      outcome: "cancelled",
    };
    const commit = commandCommit(rebound, terminal, [
      {
        type: "reconciled",
        attempt: record.dispatch,
        resolution: "terminal",
      },
      { type: "terminal", outcome: "cancelled" },
    ]);
    unwrap(await store.commit({ ...commit, reconciliations: [proof] }));
    const settled = unwrap(
      await store.command(rebound.namespace, pending.command.commandId),
    );
    assert.equal(settled.state, "terminal");
    assert.equal(settled.outcome, "cancelled");
    assert.equal(settled.dispatch.attemptId, pending.attempt.attemptId);
    assert.equal(
      first.requests.length,
      1,
      "recovery must not resend the prompt",
    );
    assert.equal(
      recoveryRuntime.methods.filter((method) => method === "thread/resume")
        .length,
      1,
    );
    assert.equal(
      recoveryRuntime.methods.filter((method) => method === "turn/start")
        .length,
      0,
      "recovery must not start another native turn",
    );
  },
);
