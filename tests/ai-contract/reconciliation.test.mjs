import { activeStage } from "../../packages/ai-contract/dist/index.js";
import assert from "node:assert/strict";
import test from "node:test";
import { decode } from "../../packages/ai-contract/dist/index.js";
import {
  VerifiedProviderSession,
  workspaceIdentity,
} from "../../packages/ai-contract/dist/session.js";
import {
  MemorySessionStore,
  fixtureSession,
  fixtureLimits,
  acceptance,
  dispatchCommand,
  commandCommit,
  verifiedReconciliation,
  seedInteraction,
  unwrap,
} from "../../packages/ai-contract/dist/testing/index.js";
const budget = () => ({
  timeoutMs: 1000,
  signal: new AbortController().signal,
});
async function pending() {
  const store = new MemorySessionStore(),
    initial = fixtureSession();
  unwrap(await store.create(initial));
  unwrap(await store.accept(acceptance(initial)));
  return {
    store,
    ...(await dispatchCommand(
      store,
      unwrap(await store.session(initial.namespace)),
      "command-1",
      "unknown",
    )),
  };
}
function reset(session, record, proof) {
  const { dispatch: _dispatch, ...accepted } = record;
  const batch = commandCommit(session, { ...accepted, state: "accepted" }, [
    {
      type: "reconciled",
      attempt: record.dispatch,
      resolution: "not_submitted",
    },
  ]);
  batch.events[0].attemptId = record.dispatch.attemptId;
  return { ...batch, nowMs: 1, providerFacts: [proof] };
}
test("verified observation cannot be copied, altered, or used across namespace and original records", async () => {
  const { store, session, record } = await pending();
  const proof = await verifiedReconciliation(session, record, "not_submitted");
  const altered = proof.observation;
  altered.status = "terminal";
  assert.equal(proof.observation.status, "not_submitted");
  for (const fake of [{ ...proof }, JSON.parse(JSON.stringify(proof)), null])
    assert.equal((await store.commit(reset(session, record, fake))).ok, false);
  const other = {
    ...session,
    namespace: { ...session.namespace, tenantId: "other" },
  };
  const cross = await verifiedReconciliation(
    other,
    { ...record, receipt: { ...record.receipt, namespace: other.namespace } },
    "not_submitted",
  );
  assert.equal((await store.commit(reset(session, record, cross))).ok, false);
  const stale = await verifiedReconciliation(
    session,
    {
      ...record,
      dispatch: { ...record.dispatch, correlationId: "different-evidence" },
    },
    "not_submitted",
  );
  assert.equal((await store.commit(reset(session, record, stale))).ok, false);
  unwrap(await store.commit(reset(session, record, proof)));
});
test("reconcile snapshots caller data and validates the exact admitted observer and provider result", async () => {
  const { store, session, record } = await pending();
  let release,
    entered,
    calls = 0,
    observation;
  const gate = new Promise((r) => (release = r)),
    started = new Promise((r) => (entered = r));
  const original = structuredClone(record);
  const port = {
    createSession: async () => ({
      ok: true,
      value: {
        binding: activeStage(session).binding,
        capabilities: activeStage(session).capabilities,
      },
    }),
    close: async () => ({ ok: true, value: { processStopped: true } }),
    reconcile: async (binding, input) => {
      calls++;
      entered();
      await gate;
      assert.deepEqual(input, original);
      return {
        ok: true,
        value: observation ?? {
          binding,
          commandId: input.command.commandId,
          attemptId: input.dispatch.attemptId,
          status: "not_submitted",
        },
      };
    },
  };
  const admitted = unwrap(
    await VerifiedProviderSession.open(
      port,
      {
        namespace: session.namespace,
        provider: activeStage(session).binding.provider,
        config: activeStage(session).binding.config,
        accountRef: activeStage(session).binding.accountRef,
        workingDirectory: ".",
        permissions: "tools_disabled",
      },
      budget(),
    ),
  );
  assert.equal(
    (
      await admitted.reconcile(
        { ...session, namespace: { ...session.namespace, tenantId: "wrong" } },
        record,
        budget(),
      )
    ).ok,
    false,
  );
  assert.equal(calls, 0);
  const pendingProof = admitted.reconcile(session, record, budget());
  await started;
  record.dispatch.correlationId = "caller-mutation";
  release();
  const proof = unwrap(await pendingProof);
  unwrap(await store.commit(reset(session, original, proof)));
  for (const patch of [
    { attemptId: "wrong" },
    { binding: { ...activeStage(session).binding, generation: "wrong" } },
    { status: "terminal", outcome: "invented" },
    { status: "not_submitted", outcome: "completed" },
  ]) {
    observation = {
      binding: activeStage(session).binding,
      commandId: original.command.commandId,
      attemptId: original.dispatch.attemptId,
      status: "not_submitted",
      ...patch,
    };
    assert.equal(
      (await admitted.reconcile(session, original, budget())).ok,
      false,
    );
  }
});
test("callback lifetime is generation bound and workspace identity normalizes equivalent paths", async () => {
  const { interaction } = await seedInteraction(new MemorySessionStore());
  assert.throws(() =>
    decode(
      JSON.stringify({
        ...interaction,
        callbackLifetime: "provider_resumable",
      }),
      fixtureLimits,
    ),
  );
  assert.equal(workspaceIdentity("."), workspaceIdentity("./child/.."));
  assert.notEqual(workspaceIdentity("."), workspaceIdentity("../other"));
});
