import assert from "node:assert/strict";
import test from "node:test";
import { DeepSeekAdapter } from "../../../packages/ai-adapters/deepseek/dist/adapter.js";
import {
  COMPOSITION_ID,
  ACTIVE_PROFILE_ID,
} from "../../../packages/ai-adapters/deepseek/dist/assembly.js";
import { VerifiedProviderSession } from "../../../packages/ai-contract/dist/session.js";
import {
  fixtureCommand,
  fixtureAttempt,
  fixtureDispatchedRecord,
  fixtureSession,
  unwrap,
} from "../../../packages/ai-contract/dist/testing/index.js";
import { budget, configuration, scriptedAdapter } from "./support.mjs";

test("admission rejects wrong native activation state and unobserved composition", async () => {
  const original = scriptedAdapter(),
    c = configuration();
  const first = unwrap(await original.createSession(c, budget()));
  await original.close(budget());
  for (const restore of [false, true]) {
    for (const patch of [
      { observationOnly: !restore },
      { observationOnly: undefined },
      { activation: "unobserved" },
    ]) {
      const p = scriptedAdapter("submitted", {
        async call(_op, value) {
          return {
            composition: COMPOSITION_ID,
            nativeSessionId: value.nativeSessionId,
            observationOnly: restore,
            activation: ACTIVE_PROFILE_ID,
            ...patch,
          };
        },
      });
      assert.equal(
        (
          await (restore
            ? p.resume(first.binding, c, budget())
            : p.createSession(c, budget()))
        ).ok,
        false,
      );
      await p.close(budget());
    }
  }
});

test("binding and attempt identity fence before native submission", async () => {
  const p = scriptedAdapter(),
    c = configuration(),
    admitted = unwrap(await VerifiedProviderSession.open(p, c, budget())),
    b = admitted.binding,
    cmd = fixtureCommand(),
    a = fixtureAttempt(b, cmd);
  for (const patch of [
    { originGeneration: "old" },
    { observerGeneration: "old" },
    { nativeSessionId: "foreign" },
    { certainty: "submitted" },
    { attemptId: "bad space" },
  ])
    assert.equal(
      (await p.submit(b, cmd, { ...a, ...patch }, budget())).certainty,
      "not_sent",
    );
  for (const patch of [
    { generation: "old" },
    { workspaceId: "other" },
    { accountRef: "other" },
  ])
    assert.equal(
      (await p.submit({ ...b, ...patch }, cmd, a, budget())).certainty,
      "not_sent",
    );
  assert.equal(
    (await p.submit(b, { ...cmd, sessionId: "foreign" }, a, budget()))
      .certainty,
    "not_sent",
  );
  assert.equal((await p.submit(b, cmd, a, budget())).certainty, "submitted");
  assert.equal((await p.createSession(c, budget())).ok, false);
  assert.equal((await p.resume(b, c, budget())).ok, false);
  await p.close(budget());
});
test("unknown correlation is attempt-specific, survives records and cannot authorize retry", async () => {
  const p = scriptedAdapter("unknown"),
    c = configuration(),
    admitted = unwrap(await VerifiedProviderSession.open(p, c, budget())),
    b = admitted.binding,
    cmd = fixtureCommand(),
    a = fixtureAttempt(b, cmd);
  const sent = await p.submit(b, cmd, a, budget());
  assert.equal(sent.certainty, "unknown");
  const record = {
    ...fixtureDispatchedRecord(b, cmd),
    dispatch: { ...a, certainty: "unknown", correlationId: sent.correlationId },
  };
  const session = {
    ...fixtureSession(),
    binding: b,
    capabilities: admitted.capabilities,
  };
  assert.equal(
    unwrap(
      await admitted.reconcile(
        session,
        JSON.parse(JSON.stringify(record)),
        budget(),
      ),
    ).observation.status,
    "unknown",
  );
  assert.equal(
    (await p.submit(b, cmd, { ...a, attemptId: "new-attempt" }, budget()))
      .certainty,
    "not_sent",
  );
  assert.equal(
    (
      await admitted.reconcile(
        session,
        {
          ...record,
          dispatch: { ...record.dispatch, attemptId: "other-attempt" },
        },
        budget(),
      )
    ).ok,
    false,
  );
  assert.deepEqual(
    await Array.fromAsync(p.observe({ ...b, generation: "old" }, budget())),
    [],
  );
  await p.close(budget());
});
test("restore rejects tenant, workspace and configuration drift before creating child", async () => {
  const original = scriptedAdapter(),
    c = configuration(),
    admitted = unwrap(
      await VerifiedProviderSession.open(original, c, budget()),
    );
  await original.close(budget());
  const session = {
    ...fixtureSession(),
    binding: admitted.binding,
    capabilities: admitted.capabilities,
  };
  for (const changed of [
    { ...c, namespace: { ...c.namespace, tenantId: "other" } },
    { ...c, workingDirectory: "/tmp/other" },
  ]) {
    const p = scriptedAdapter();
    assert.equal(
      (await VerifiedProviderSession.restore(p, session, changed, budget())).ok,
      false,
    );
  }
  const p = scriptedAdapter();
  assert.equal(
    (
      await p.resume(
        { ...session.binding, config: { id: "drift", revision: "2" } },
        c,
        budget(),
      )
    ).ok,
    false,
  );
  await p.close(budget());
});
test("late resolver cannot spawn after close; incomplete child cleanup is preserved by admission", async () => {
  let release,
    spawned = 0;
  const c = configuration(),
    resolved = {
      configuration: c,
      persistenceDirectory: "/tmp/dsh",
      apiKey: "fixture",
      model: "deepseek-chat",
    };
  const gate = new Promise((r) => (release = r)),
    p = new DeepSeekAdapter(
      {
        resolveConfiguration: async () => {
          await gate;
          return resolved;
        },
      },
      () => {
        spawned++;
        throw Error("must not start");
      },
    );
  const abort = new AbortController(),
    opening = VerifiedProviderSession.open(p, c, {
      ...budget(),
      signal: abort.signal,
    });
  abort.abort();
  const result = await opening;
  assert.equal(result.ok, false);
  assert.equal(result.cleanupError, undefined);
  release();
  await new Promise((r) => setImmediate(r));
  assert.equal(spawned, 0);
  const broken = new DeepSeekAdapter(
    { resolveConfiguration: async () => resolved },
    () => ({
      onEvent() {},
      stopped: new Promise(() => {}),
      stop() {},
      async call() {
        throw Error("initialize failed");
      },
    }),
  );
  const failed = await VerifiedProviderSession.open(broken, c, budget(20));
  assert.equal(failed.ok, false);
  assert.equal(failed.cleanupError.code, "unavailable");
});

// Persisted identities must be independent of JSON object member order.
test("native scope and attempt hashes are canonical", async () => {
  const { digest } = await import(
    "../../../packages/ai-adapters/deepseek/dist/configuration.js"
  );
  assert.equal(
    digest({ a: 1, nested: { b: 2, c: 3 } }),
    digest({ nested: { c: 3, b: 2 }, a: 1 }),
  );
  const p = scriptedAdapter(),
    c = configuration(),
    admitted = unwrap(await VerifiedProviderSession.open(p, c, budget()));
  await p.close(budget());
  const reordered = {
    ...c,
    namespace: Object.fromEntries(Object.entries(c.namespace).reverse()),
    config: { revision: "1", id: "config-1" },
  };
  const next = scriptedAdapter();
  assert.equal(
    (await next.resume(admitted.binding, reordered, budget())).ok,
    true,
  );
  await next.close(budget());
});

test("diagnostics retain only closed stage/reason and never surface resolver exceptions", async () => {
  const entries = [],
    p = new DeepSeekAdapter({
      onDiagnostic: (entry) => {
        entries.push(entry);
        throw Error("sink failure");
      },
      resolveConfiguration: async () => {
        throw Error("credential contents and private path");
      },
    });
  assert.equal((await p.createSession(configuration(), budget())).ok, false);
  assert.deepEqual(entries, [
    { stage: "initialize", reason: "native_failure", generation: undefined },
  ]);
  assert.equal((await p.close(budget())).ok, true);
});

test("raw port admission also requires a complete trusted namespace before spawn", async () => {
  for (const namespace of [
    undefined,
    {},
    { ...configuration().namespace, tenantId: "" },
  ]) {
    const c = { ...configuration(), namespace };
    let spawned = false;
    const p = new DeepSeekAdapter(
      {
        resolveConfiguration: async () => ({
          configuration: c,
          persistenceDirectory: "/tmp/dsh",
          apiKey: "fixture",
          model: "deepseek-chat",
        }),
      },
      () => {
        spawned = true;
        throw Error("should not spawn");
      },
    );
    assert.equal((await p.createSession(c, budget())).ok, false);
    assert.equal(spawned, false);
    await p.close(budget());
  }
});

test("initialize NativeFault produces one sanitized operation diagnostic", async () => {
  const { NativeFault } = await import(
    "../../../packages/ai-adapters/deepseek/dist/protocol.js"
  );
  const diagnostics = [],
    c = configuration();
  const p = new DeepSeekAdapter(
    {
      onDiagnostic: (d) => diagnostics.push(d),
      resolveConfiguration: async () => ({
        configuration: c,
        persistenceDirectory: "/tmp/dsh",
        apiKey: "fixture",
        model: "deepseek-chat",
      }),
    },
    () => ({
      onEvent() {},
      stop() {},
      stopped: Promise.resolve(),
      call: async () => {
        throw new NativeFault("dependency_drift");
      },
    }),
  );
  assert.equal((await p.createSession(c, budget())).ok, false);
  assert.equal(diagnostics.length, 1);
  assert.equal(diagnostics[0].stage, "initialize");
  assert.equal(diagnostics[0].reason, "dependency_drift");
  await p.close(budget());
});
