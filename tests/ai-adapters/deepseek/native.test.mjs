import {
  startStage,
  providerStage,
} from "../../../packages/ai-contract/dist/index.js";
import { replaceStage } from "../../../packages/ai-contract/dist/index.js";
import { activeStage } from "../../../packages/ai-contract/dist/index.js";
import assert from "node:assert/strict";
import test from "node:test";
import { fingerprint } from "../../../packages/ai-contract/dist/index.js";
import { VerifiedProviderSession } from "../../../packages/ai-contract/dist/session.js";
import {
  fixtureSession,
  fixtureAttempt,
  fixtureLimits,
  unwrap,
  MemorySessionStore,
  acceptance,
  commandCommit,
  terminalCommit,
} from "../../../packages/ai-contract/dist/testing/index.js";
import { budget } from "./support.mjs";
import {
  environment,
  completion,
  command,
  collect,
  files,
} from "./native-support.mjs";
test("real Harness process: true deltas, durable terminal, cold read and native multi-turn continuation", async (t) => {
  const env = await environment(t, (_body, res) => completion(res));
  const first = env.port(),
    admitted = unwrap(
      await VerifiedProviderSession.open(
        first,
        env.config,
        budget(),
        env.admission,
      ),
    );
  const c = command(),
    a = fixtureAttempt(admitted.binding, c);
  const sent = await first.dispatch(admitted.binding, c, a, budget());
  assert.equal(sent.certainty, "submitted");
  const events = await collect(first, sent.binding);
  assert.ok(
    events.filter((e) => e.type === "delta").length >= 2,
    JSON.stringify(events),
  );
  assert.equal(
    events.find((e) => e.type === "event" && e.body.type === "text")?.body.text,
    "native answer",
  );
  assert.equal(
    events.at(-1)?.body?.outcome,
    "completed",
    JSON.stringify(events),
  );
  assert.equal(
    (
      await first.dispatch(
        sent.binding,
        c,
        { ...a, attemptId: "replacement-attempt" },
        budget(),
      )
    ).certainty,
    "not_sent",
  );
  assert.equal(
    env.requests.length,
    1,
    "a completed command cannot acquire another native attempt",
  );
  await first.close(budget());
  const before = await files(env.dir);
  assert.ok(Object.keys(before).length);
  const previous = startStage(
    { ...fixtureSession(), namespace: env.config.namespace, stages: [] },
    providerStage(sent.binding, admitted.capabilities),
  );
  const second = env.port(),
    restored = unwrap(
      await VerifiedProviderSession.restore(
        second,
        previous,
        env.config,
        budget(),
        env.admission,
      ),
    );
  assert.notEqual(restored.binding.generation, admitted.binding.generation);
  assert.deepEqual(
    await files(env.dir),
    before,
    "restore must not repair or append logs",
  );
  assert.equal(env.requests.length, 1, "restore must not request a model");
  const record = {
    schemaVersion: 5,
    kind: "commandRecord",
    command: c,
    receipt: {
      schemaVersion: 5,
      kind: "receipt",
      stageId: previous.currentStageId,
      namespace: env.config.namespace,
      commandId: c.commandId,
      contentHash: fingerprint(c, fixtureLimits),
      acceptedAtMs: 0,
      retryUntilMs: c.expiresAtMs,
      receiptUntilMs: c.expiresAtMs,
      acceptedRevision: 1,
    },
    state: "reconciliation_required",
    dispatch: {
      ...a,
      observerGeneration: restored.binding.generation,
      certainty: "submitted",
      nativeRequestId: sent.binding.nativeRequestId,
    },
  };
  const current = replaceStage({ ...previous }, restored.binding, undefined);
  const proof = unwrap(await restored.reconcile(current, record, budget()));
  assert.equal(proof.observation.status, "terminal");
  assert.deepEqual(await files(env.dir), before);
  const next = command("command-2", "what did I say before?");
  const result = await second.dispatch(
    restored.binding,
    next,
    fixtureAttempt(restored.binding, next),
    budget(),
  );
  assert.equal(result.certainty, "submitted");
  assert.equal(
    (await collect(second, result.binding)).at(-1)?.body?.outcome,
    "completed",
  );
  assert.equal(env.requests.length, 2);
  assert.ok(
    env.requests[1].messages.some(
      (m) =>
        m.role === "user" && JSON.stringify(m.content).includes("say hello"),
    ),
  );
  assert.ok(
    env.requests[1].messages.some(
      (m) =>
        m.role === "assistant" &&
        JSON.stringify(m.content).includes("native answer"),
    ),
  );
});

test("terminal commit -> restore/rebind -> new command without reconciling settled records", async (t) => {
  const env = await environment(t, (_b, res) => completion(res)),
    p = env.port();
  const admitted = unwrap(
    await VerifiedProviderSession.open(p, env.config, budget(), env.admission),
  );
  const store = new MemorySessionStore(),
    initial = startStage(
      { ...fixtureSession(), namespace: env.config.namespace, stages: [] },
      providerStage(admitted.binding, admitted.capabilities),
    );
  unwrap(await store.create(initial));
  const c = command();
  unwrap(await store.accept(acceptance(initial, c)));
  let head = unwrap(await store.session(initial.namespace));
  let record = unwrap(await store.command(head.namespace, c.commandId));
  const attempt = fixtureAttempt(activeStage(head).binding, c);
  record = { ...record, state: "dispatching", dispatch: attempt };
  unwrap(
    await store.commit(
      commandCommit(head, record, [
        { type: "dispatch", attempt },
        { type: "status", state: "dispatching" },
      ]),
    ),
  );
  const dispatched = unwrap(await admitted.dispatch(head, record, budget()));
  assert.equal(dispatched.observation.status, "submitted");
  const sent = { binding: dispatched.observation.binding };
  head = unwrap(await store.session(head.namespace));
  record = {
    ...record,
    state: "dispatching",
    dispatch: {
      ...attempt,
      certainty: "submitted",
      nativeRequestId: sent.binding.nativeRequestId,
    },
  };
  const committed = commandCommit(head, record, [
    { type: "reconciled", attempt, resolution: "submitted" },
    { type: "dispatch", attempt: record.dispatch },
  ]);
  activeStage(committed.session).binding = sent.binding;
  committed.providerFacts = [dispatched];
  unwrap(await store.commit(committed));
  assert.equal(
    (await collect(p, sent.binding)).at(-1)?.body?.outcome,
    "completed",
  );
  head = unwrap(await store.session(head.namespace));
  unwrap(
    await store.commit(
      await terminalCommit(
        head,
        record,
        unwrap(await admitted.reconcile(head, record, budget())),
      ),
    ),
  );
  await p.close(budget());
  const before = await files(env.dir),
    previous = unwrap(await store.session(head.namespace)),
    next = env.port();
  const restored = unwrap(
    await VerifiedProviderSession.restore(
      next,
      previous,
      env.config,
      budget(),
      env.admission,
    ),
  );
  unwrap(
    await store.rebind({
      namespace: head.namespace,
      expectedRevision: previous.revision,
      expectedGeneration: activeStage(previous).binding.generation,
      restored,
      eventId: "settled-rebind",
    }),
  );
  head = unwrap(await store.session(head.namespace));
  record = unwrap(await store.command(head.namespace, c.commandId));
  assert.equal(record.state, "terminal");
  assert.equal(
    (await restored.reconcile(head, record, budget())).ok,
    false,
    "settled records cannot obtain reconciliation proofs",
  );
  assert.deepEqual(await files(env.dir), before);
  assert.equal(env.requests.length, 1);
  const c2 = command("after-settled");
  const sent2 = await next.dispatch(
    activeStage(head).binding,
    c2,
    fixtureAttempt(activeStage(head).binding, c2),
    budget(),
  );
  assert.equal(sent2.certainty, "submitted");
  assert.equal(
    (await collect(next, sent2.binding)).at(-1)?.body?.outcome,
    "completed",
  );
});

test("real native evidence through restore -> rebind -> reconcile -> atomic store commit", async (t) => {
  const env = await environment(t, (_b, res) => completion(res));
  const p = env.port(),
    admitted = unwrap(
      await VerifiedProviderSession.open(
        p,
        env.config,
        budget(),
        env.admission,
      ),
    );
  const store = new MemorySessionStore(),
    initial = startStage(
      { ...fixtureSession(), namespace: env.config.namespace, stages: [] },
      providerStage(admitted.binding, admitted.capabilities),
    );
  unwrap(await store.create(initial));
  const c = command();
  unwrap(await store.accept(acceptance(initial, c)));
  let head = unwrap(await store.session(initial.namespace));
  let record = unwrap(await store.command(head.namespace, c.commandId));
  const attempt = fixtureAttempt(activeStage(head).binding, c),
    preparing = { ...record, state: "dispatching", dispatch: attempt };
  unwrap(
    await store.commit(
      commandCommit(head, preparing, [
        { type: "dispatch", attempt },
        { type: "status", state: "dispatching" },
      ]),
    ),
  );
  const sent = await p.dispatch(
    activeStage(head).binding,
    c,
    attempt,
    budget(),
  );
  assert.equal(sent.certainty, "submitted");
  await collect(p, sent.binding);
  head = unwrap(await store.session(head.namespace));
  const unknown = {
    ...preparing,
    state: "reconciliation_required",
    dispatch: {
      ...attempt,
      certainty: "unknown",
      correlationId: sent.binding.nativeRequestId,
    },
  };
  unwrap(
    await store.commit(
      commandCommit(head, unknown, [
        { type: "dispatch", attempt: unknown.dispatch },
        { type: "status", state: "reconciliation_required" },
      ]),
    ),
  );
  await p.close(budget());
  const previous = unwrap(await store.session(head.namespace)),
    restoredPort = env.port();
  const restored = unwrap(
    await VerifiedProviderSession.restore(
      restoredPort,
      previous,
      env.config,
      budget(),
      env.admission,
    ),
  );
  assert.equal(
    (
      await store.rebind({
        namespace: head.namespace,
        expectedRevision: previous.revision,
        expectedGeneration: activeStage(previous).binding.generation,
        restored: JSON.parse(JSON.stringify(restored)),
        eventId: "forged",
      })
    ).ok,
    false,
  );
  unwrap(
    await store.rebind({
      namespace: head.namespace,
      expectedRevision: previous.revision,
      expectedGeneration: activeStage(previous).binding.generation,
      restored,
      eventId: "rebind-native",
    }),
  );
  head = unwrap(await store.session(head.namespace));
  record = unwrap(await store.command(head.namespace, c.commandId));
  assert.equal(record.dispatch.attemptId, attempt.attemptId);
  assert.equal(record.dispatch.originGeneration, attempt.originGeneration);
  assert.equal(record.dispatch.correlationId, unknown.dispatch.correlationId);
  assert.equal(
    (await restored.reconcile(previous, unknown, budget())).ok,
    false,
  );
  const proof = unwrap(await restored.reconcile(head, record, budget()));
  assert.equal(proof.observation.status, "terminal");
  const batch = await terminalCommit(head, record, proof);
  assert.equal(
    (
      await store.commit({
        ...batch,
        providerFacts: [JSON.parse(JSON.stringify(proof))],
      })
    ).ok,
    false,
  );
  unwrap(await store.commit(batch));
  assert.equal(
    unwrap(await store.command(head.namespace, c.commandId)).state,
    "terminal",
  );
});

test("request checkpoint precedes HTTP dispatch; crash and synthetic interrupted remain unknown without log repair", async (t) => {
  let called;
  const requested = new Promise((r) => (called = r));
  const env = await environment(t, (_b, _res) => {
    called();
  });
  const p = env.port(),
    admitted = unwrap(
      await VerifiedProviderSession.open(
        p,
        env.config,
        budget(),
        env.admission,
      ),
    );
  const c = command(),
    a = fixtureAttempt(admitted.binding, c),
    sent = await p.dispatch(admitted.binding, c, a, budget());
  assert.equal(sent.certainty, "submitted");
  await requested;
  const atEffect = Object.values(await files(env.dir)).join("\n");
  assert.match(atEffect, /request\/header/);
  assert.match(atEffect, /user\/message/);
  await p.close(budget());
  const logs = await files(env.dir),
    previous = startStage(
      { ...fixtureSession(), namespace: env.config.namespace, stages: [] },
      providerStage(sent.binding, admitted.capabilities),
    );
  const replacement = env.port(),
    admitted2 = unwrap(
      await VerifiedProviderSession.restore(
        replacement,
        previous,
        env.config,
        budget(),
        env.admission,
      ),
    );
  const record = {
    schemaVersion: 5,
    kind: "commandRecord",
    command: c,
    receipt: {
      schemaVersion: 5,
      kind: "receipt",
      stageId: previous.currentStageId,
      namespace: env.config.namespace,
      commandId: c.commandId,
      contentHash: fingerprint(c, fixtureLimits),
      acceptedAtMs: 0,
      retryUntilMs: c.expiresAtMs,
      receiptUntilMs: c.expiresAtMs,
      acceptedRevision: 1,
    },
    state: "reconciliation_required",
    dispatch: {
      ...a,
      observerGeneration: admitted2.binding.generation,
      certainty: "unknown",
      correlationId: sent.binding.nativeRequestId,
    },
  };
  const proof = unwrap(
    await admitted2.reconcile(
      replaceStage({ ...previous }, admitted2.binding, undefined),
      record,
      budget(),
    ),
  );
  assert.equal(proof.observation.status, "unknown");
  const next = command("next");
  assert.equal(
    (
      await replacement.dispatch(
        admitted2.binding,
        next,
        fixtureAttempt(admitted2.binding, next),
        budget(),
      )
    ).certainty,
    "not_sent",
  );
  assert.deepEqual(await files(env.dir), logs);
  assert.equal(env.requests.length, 1);
});

test("fresh admission checkpoints an empty session before immediate close and cold restore", async (t) => {
  const env = await environment(t, (_body, res) => completion(res));
  const p = env.port(),
    admitted = unwrap(
      await VerifiedProviderSession.open(
        p,
        env.config,
        budget(),
        env.admission,
      ),
    );
  unwrap(await p.close(budget()));
  const before = await files(env.dir);
  assert.ok(
    Object.keys(before).length > 0,
    "admission must persist native session identity",
  );
  const second = env.port();
  const restored = unwrap(
    await VerifiedProviderSession.restore(
      second,
      startStage(
        { ...fixtureSession(), namespace: env.config.namespace, stages: [] },
        providerStage(admitted.binding, admitted.capabilities),
      ),
      env.config,
      budget(),
      env.admission,
    ),
  );
  assert.equal(
    restored.binding.nativeSessionId,
    admitted.binding.nativeSessionId,
  );
  assert.deepEqual(await files(env.dir), before);
  assert.equal(env.requests.length, 0);
});

test("real worker restoration fault survives IPC and budget classification", async (t) => {
  const { nativeRuntime } = await import(
    "../../../packages/ai-adapters/deepseek/dist/runtime.js"
  );
  const { NativeFault } = await import(
    "../../../packages/ai-adapters/deepseek/dist/protocol.js"
  );
  const { COMPOSITION_ID } = await import(
    "../../../packages/ai-adapters/deepseek/dist/assembly.js"
  );
  const { digest, identity } = await import(
    "../../../packages/ai-adapters/deepseek/dist/configuration.js"
  );
  const env = await environment(t, (_b, res) => completion(res)),
    p = env.port();
  const admitted = unwrap(
    await VerifiedProviderSession.open(p, env.config, budget(), env.admission),
  );
  await p.close(budget());
  const before = await files(env.dir),
    runtime = nativeRuntime();
  try {
    await assert.rejects(
      runtime.call(
        "initialize",
        {
          nativeSessionId: admitted.binding.nativeSessionId,
          workingDirectory: process.cwd(),
          persistenceDirectory: env.dir,
          scope: digest(identity(env.config)),
          model: "deepseek-chat",
          apiUrl: "https://custom.example.test/v1",
          apiKey: "fixture",
          apiUrl: "https://api.deepseek.com",
          controlled: false,
          restore: true,
          composition: COMPOSITION_ID,
        },
        budget(),
      ),
      (error) =>
        error instanceof NativeFault && error.reason === "restoration_failed",
    );
    assert.deepEqual(await files(env.dir), before);
    assert.equal(env.requests.length, 0);
  } finally {
    runtime.stop();
    await runtime.stopped;
  }
});
