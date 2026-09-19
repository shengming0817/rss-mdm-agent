import assert from "node:assert/strict";
import test from "node:test";
import { join } from "node:path";
import { VerifiedProviderSession } from "../../packages/ai-contract/dist/session.js";
import {
  fixtureSession,
  fixtureCommand,
  acceptance,
  dispatchCommand,
  terminalCommit,
  emptyCommit,
  restoredSession,
  unwrap,
} from "../../packages/ai-contract/dist/testing/index.js";
import { harness, budget } from "./support.mjs";

test("SQLite rebind retains native thread and all steer attempts; terminal closure covers the whole turn", async (t) => {
  const h = harness(t),
    path = join(h.directory, "threads.sqlite"),
    initial = fixtureSession();
  initial.binding.nativeThreadId = "owned-thread";
  initial.capabilities.steer = "supported";
  initial.capabilities.continuation = "across_processes";
  let store = unwrap(h.open(path)),
    head = initial;
  unwrap(await store.create(initial));
  for (const [id, request] of [
    ["start", "start-request"],
    ["steer-1", "request-1"],
    ["steer-2", "request-2"],
  ]) {
    const command = fixtureCommand(id);
    if (id !== "start")
      command.input = {
        type: "prompt",
        policy: "steer",
        text: id,
        targetRunId: "one-turn",
      };
    unwrap(await store.accept(acceptance(head, command)));
    head = unwrap(await store.session(initial.namespace));
    ({ session: head } = await dispatchCommand(store, head, id, "submitted", {
      nativeRunId: "one-turn",
      nativeRequestId: request,
    }));
  }
  const before = unwrap(
    await store.snapshotPage(initial.namespace, { limit: 64 }),
  );
  unwrap(await store.close(budget()));
  store = unwrap(h.open(path, "open"));
  assert.deepEqual(
    unwrap(await store.snapshotPage(initial.namespace, { limit: 64 })).commands,
    before.commands,
  );
  head = unwrap(await store.session(initial.namespace));
  const restored = await restoredSession(head, "fresh-process");
  head = unwrap(
    await store.rebind({
      namespace: head.namespace,
      expectedRevision: head.revision,
      expectedGeneration: head.binding.generation,
      restored,
      eventId: "restore",
    }),
  );
  for (const id of ["start", "steer-1", "steer-2"]) {
    const record = unwrap(await store.command(head.namespace, id));
    assert.equal(record.dispatch.nativeThreadId, "owned-thread");
    assert.equal(record.dispatch.originGeneration, initial.binding.generation);
    assert.equal(record.dispatch.observerGeneration, "fresh-process");
    const clear = emptyCommit(head);
    clear.session.binding = { ...head.binding };
    delete clear.session.binding.nativeRunId;
    delete clear.session.binding.nativeRequestId;
    assert.equal((await store.commit(clear)).ok, false);
    const port = {
      createSession: async () => ({
        ok: true,
        value: { binding: head.binding, capabilities: head.capabilities },
      }),
      close: async () => ({ ok: true, value: { processStopped: true } }),
      reconcile: async () => ({
        ok: true,
        value: {
          commandId: id,
          attemptId: record.dispatch.attemptId,
          binding: {
            ...head.binding,
            nativeRequestId: record.dispatch.nativeRequestId,
          },
          status: "terminal",
          outcome: "completed",
        },
      }),
    };
    const admitted = unwrap(
      await VerifiedProviderSession.open(
        port,
        {
          namespace: head.namespace,
          provider: head.binding.provider,
          config: head.binding.config,
          accountRef: head.binding.accountRef,
          workingDirectory: ".",
          permissions: "tools_disabled",
        },
        budget(),
      ),
    );
    const proof = unwrap(await admitted.reconcile(head, record, budget()));
    unwrap(await store.commit(terminalCommit(head, record, proof)));
    head = unwrap(await store.session(head.namespace));
  }
  const clear = emptyCommit(head);
  clear.session.binding = { ...head.binding };
  delete clear.session.binding.nativeRunId;
  delete clear.session.binding.nativeRequestId;
  unwrap(await store.commit(clear));
  const snapshot = unwrap(
    await store.snapshotPage(initial.namespace, { limit: 128 }),
  );
  assert.equal(snapshot.commands.length, 3);
  assert.equal(
    snapshot.commands.every(
      (record) =>
        record.state === "terminal" &&
        record.dispatch.nativeThreadId === "owned-thread",
    ),
    true,
  );
  assert.equal(
    snapshot.events
      .filter((event) => event.body.type === "dispatch")
      .every((event) => event.body.attempt.nativeThreadId === "owned-thread"),
    true,
  );
});
