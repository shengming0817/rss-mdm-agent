import { fixturePersistence } from "./harness.mjs";
import { workerRuntime } from "./worker-runtime.mjs";
import assert from "node:assert/strict";
import test from "node:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createHost, HostFailure } from "../../packages/ai-host/dist/index.js";
import { openSqliteStore } from "../../packages/ai-store-sqlite/dist/index.js";
import { activeStage } from "../../packages/ai-contract/dist/index.js";
import { connectionPersistence } from "../../apps/ai-host/dist/secrets.js";
import {
  callerFor,
  callerAvailable,
  requireLocalExecution,
  closeOwners,
  suspendNativeCaller,
} from "../../apps/ai-host/dist/index.js";
const caller = {
  tenantId: "test-users",
  principalId: "alice",
  authorityId: "desktop-fixture",
};
const budget = () => ({
  timeoutMs: 5000,
  signal: new AbortController().signal,
});
const unwrap = (result) => {
  assert.equal(result.ok, true, JSON.stringify(result));
  return result.value;
};
const connection = (id) => ({
  schemaVersion: 7,
  kind: "connection",
  connectionId: id,
  name: id,
  provider: "codex",
  configRevision: 1,

  profile: "conversation",
  status: "ready",
  source: {
    type: "custom_api",
    apiUrl: "https://example.invalid/v1",
    model: "test",
  },
});
const draftOf = ({ connectionId, name, provider, profile, source }) => ({
  connectionId,
  name,
  provider,
  profile,
  source,
});
const until = async (action) => {
  for (let i = 0; i < 200; i++) {
    if (await action()) return;
    await new Promise((r) => setTimeout(r, 10));
  }
  assert.fail("condition timed out");
};
test("real Host lazily opens phases, drains accepted work before switching, and replays receipts before opening providers", async (t) => {
  const root = await mkdtemp(join(tmpdir(), "rss-phases-"));
  const store = unwrap(
    openSqliteStore({ path: join(root, "ai.sqlite"), mode: "create" }),
  );
  let opened = 0,
    disposed = 0;
  const host = unwrap(
    await createHost({
      credentialPersistence: fixturePersistence(store),
      workerRuntime,
      store,
      launchFences: store,
      delivery: null,
      resolve: async (_caller, options, namespace) => {
        opened++;
        return {
          dispose: async () => {
            disposed++;
          },
          configuration: {
            namespace,
            provider: options.provider,
            config: options.config,

            workingDirectory: root,
            permissions: "tools_disabled",
          },
          artifact: new URL("./provider.mjs", import.meta.url).href,
        };
      },
    }),
  );
  t.after(async () => {
    unwrap(await host.close(budget()));
    await rm(root, { recursive: true, force: true });
  });
  const empty = unwrap(await host.createSession(caller, {}, budget()));
  assert.equal(opened, 0);
  assert.deepEqual(empty.stages, []);
  const command = (id, text = "quick") => ({
    schemaVersion: 7,
    kind: "command",
    sessionId: empty.namespace.sessionId,
    commandId: id,
    expiresAtMs: Date.now() + 60000,
    input: { type: "prompt", policy: "queue_next", text },
  });
  assert.equal(
    (await host.submit(caller, command("no-connection"), budget())).error.code,
    "connection_required",
  );
  for (const id of ["one", "two"])
    unwrap(await store.saveConnection(caller, connection(id), null));
  unwrap(
    await host.selectConnection(
      caller,
      empty.namespace.sessionId,
      "one",
      budget(),
    ),
  );
  const first = command("first", "hold"),
    receipt = unwrap(await host.submit(caller, first, budget()));
  await until(
    async () =>
      unwrap(await store.command(empty.namespace, "first")).state === "running",
  );
  const old = unwrap(await store.session(empty.namespace));
  unwrap(await host.submit(caller, command("queued"), budget()));
  unwrap(
    await host.selectConnection(
      caller,
      empty.namespace.sessionId,
      "two",
      budget(),
    ),
  );
  assert.equal(
    (await host.submit(caller, command("wait"), budget())).error.code,
    "connection_switch_pending",
  );
  assert.deepEqual(unwrap(await host.submit(caller, first, budget())), receipt);
  assert.equal(opened, 1);
  const record = unwrap(await store.command(empty.namespace, "first"));
  unwrap(
    await host.cancel(
      caller,
      {
        ...command("cancel"),
        input: {
          type: "cancel",
          targetCommandId: "first",
          generation: activeStage(old).binding.generation,
          nativeRunId: record.dispatch.nativeRunId,
        },
      },
      budget(),
    ),
  );
  await until(
    async () =>
      unwrap(await store.command(empty.namespace, "queued")).state ===
      "terminal",
  );
  const next = unwrap(await host.submit(caller, command("next"), budget()));
  assert.notEqual(next.stageId, receipt.stageId);
  assert.equal(opened, 2);
  assert.equal(disposed, 1, "the stopped first-stage snapshot is released");
  const head = unwrap(await store.session(empty.namespace));
  assert.equal(head.stages.length, 2);
  assert.equal(head.stages[0].connectionId, "one");
  assert.equal(activeStage(head).connectionId, "two");
  assert.deepEqual(unwrap(await host.submit(caller, first, budget())), receipt);
  assert.equal(opened, 2);
  await until(
    async () =>
      unwrap(await store.command(empty.namespace, "next")).state === "terminal",
  );
  const preview = unwrap(
    await host.previewHistory(
      caller,
      empty.namespace.sessionId,
      "two",
      1,
      budget(),
    ),
  );
  assert.deepEqual(preview.commandIds, ["next"]);
  unwrap(
    await host.selectConnection(
      caller,
      empty.namespace.sessionId,
      "two",
      budget(),
      true,
    ),
  );
  const carrying = {
    ...command("with-history"),
    input: { ...command("with-history").input, history: preview },
  };
  const carried = unwrap(await host.submit(caller, carrying, budget()));
  assert.notEqual(carried.stageId, next.stageId);
  assert.deepEqual(
    unwrap(await host.submit(caller, carrying, budget())),
    carried,
  );
  assert.equal(unwrap(await store.session(empty.namespace)).stages.length, 3);
  const bad = {
    ...carrying,
    commandId: "tampered",
    input: {
      ...carrying.input,
      history: { ...preview, text: "forged history" },
    },
  };
  assert.equal((await host.submit(caller, bad, budget())).ok, false);
  const page = unwrap(
    await host.snapshotPage(
      caller,
      empty.namespace.sessionId,
      { limit: 256 },
      budget(),
    ),
  );
  const { emptyView, restoreSnapshot } = await import(
    "../../packages/ai-client/dist/projection.js"
  );
  const view = emptyView(page.session, page.cursor);
  restoreSnapshot(view, [page]);
  assert.ok(
    Object.values(view.messages).some(
      (message) => message.commandId === "queued" && message.stable,
    ),
  );
  assert.ok(
    Object.values(view.messages).some(
      (message) => message.commandId === "next" && message.stable,
    ),
  );
  await until(
    async () =>
      unwrap(await store.command(empty.namespace, "with-history")).state ===
      "terminal",
  );
  const beforeDelete = unwrap(
    await host.snapshotPage(
      caller,
      empty.namespace.sessionId,
      { limit: 256 },
      budget(),
    ),
  );
  unwrap(await host.deleteConnection(caller, "two", 1, budget()));
  const afterDelete = unwrap(
    await host.snapshotPage(
      caller,
      empty.namespace.sessionId,
      { limit: 256 },
      budget(),
    ),
  );
  assert.deepEqual(
    { ...afterDelete, snapshotId: beforeDelete.snapshotId },
    beforeDelete,
  );
  assert.equal(
    unwrap(await host.listSessions(caller, { limit: 256 }, budget())).items
      .length,
    1,
  );
  const activations = opened;
  assert.equal(
    (await host.submit(caller, command("deleted-connection"), budget())).ok,
    false,
  );
  assert.equal(opened, activations);
  assert.deepEqual(
    {
      ...unwrap(
        await host.snapshotPage(
          caller,
          empty.namespace.sessionId,
          { limit: 256 },
          budget(),
        ),
      ),
      snapshotId: beforeDelete.snapshotId,
    },
    beforeDelete,
  );
  await suspendNativeCaller(host, {
    schemaVersion: 7,
    kind: "userContext",
    user: {
      schemaVersion: 7,
      kind: "testUser",
      userId: "alice",
      displayName: "Alice",
      nameKey: "alice",
    },
    generation: "restored-generation",
  });
  host.activateCaller(caller);
  const beforeDeletedResume = opened;
  assert.equal(
    (await host.resume(caller, empty.namespace.sessionId, budget())).error.code,
    "connection_required",
  );
  assert.equal(opened, beforeDeletedResume);
  unwrap(await host.close(budget()));
  assert.equal(disposed, opened, "shutdown releases the active-stage snapshot");
});

test("switching test users cancels queued model work and keeps the old user's receipts scoped", async (t) => {
  const root = await mkdtemp(join(tmpdir(), "rss-switch-user-"));
  const store = unwrap(
    openSqliteStore({ path: join(root, "ai.sqlite"), mode: "create" }),
  );
  const host = unwrap(
    await createHost({
      credentialPersistence: fixturePersistence(store),
      workerRuntime,
      store,
      launchFences: store,
      delivery: null,
      resolve: async (_caller, options, namespace) => ({
        configuration: {
          namespace,
          provider: options.provider,
          config: options.config,

          workingDirectory: root,
          permissions: "tools_disabled",
        },
        artifact: new URL("./provider.mjs", import.meta.url).href,
      }),
    }),
  );
  t.after(async () => {
    unwrap(await host.close(budget()));
    await rm(root, { recursive: true, force: true });
  });
  unwrap(await store.saveConnection(caller, connection("one"), null));
  const session = unwrap(await host.createSession(caller, {}, budget()));
  const command = (commandId, text) => ({
    schemaVersion: 7,
    kind: "command",
    sessionId: session.namespace.sessionId,
    commandId,
    expiresAtMs: Date.now() + 60000,
    input: { type: "prompt", policy: "queue_next", text },
  });
  const running = command("running", "hold"),
    queued = command("queued", "quick");
  const receipt = unwrap(await host.submit(caller, running, budget()));
  await until(
    async () =>
      unwrap(await store.command(session.namespace, "running")).state ===
      "running",
  );
  const before = unwrap(await store.command(session.namespace, "running"));
  unwrap(await host.submit(caller, queued, budget()));
  unwrap(await host.suspendCaller(caller, budget()));
  const stopped = unwrap(await store.command(session.namespace, "running"));
  assert.equal(stopped.state, "terminal");
  assert.equal(stopped.outcome, "cancelled");
  const snapshot = unwrap(
    await store.snapshotPage(session.namespace, { limit: 256 }),
  );
  const cancellation = snapshot.commands.find(
    (row) =>
      row.command.input.type === "cancel" &&
      row.command.input.targetCommandId === "running",
  );
  assert.ok(cancellation);
  assert.equal(cancellation.state, "acknowledged");
  assert.equal(
    cancellation.command.input.generation,
    before.dispatch.observerGeneration,
  );
  assert.equal(
    cancellation.command.input.nativeRunId,
    before.dispatch.nativeRunId,
  );
  assert.equal(cancellation.receipt.stageId, receipt.stageId);
  assert.equal(
    unwrap(await store.command(session.namespace, "queued")).state,
    "cancelled",
  );
  const bob = { ...caller, principalId: "bob" };
  assert.equal((await host.submit(bob, running, budget())).ok, false);
  assert.equal(
    (
      await host.snapshotPage(
        bob,
        session.namespace.sessionId,
        { limit: 256 },
        budget(),
      )
    ).ok,
    false,
  );
  assert.deepEqual(
    unwrap(await host.connections(bob, budget())).connections,
    [],
  );
  assert.equal(
    (await host.submit(caller, command("late", "quick"), budget())).ok,
    false,
  );
  assert.equal(
    (
      await host.previewHistory(
        caller,
        session.namespace.sessionId,
        "one",
        undefined,
        budget(),
      )
    ).error.code,
    "unavailable",
  );
  host.activateCaller(caller);
  assert.deepEqual(
    unwrap(await store.command(session.namespace, "running")),
    stopped,
  );
  assert.deepEqual(
    unwrap(await host.submit(caller, running, budget())),
    receipt,
  );
});

test("ordinary runtime snapshot cleanup is retained and retried after a failure", async (t) => {
  const root = await mkdtemp(join(tmpdir(), "rss-runtime-cleanup-"));
  const store = unwrap(
    openSqliteStore({ path: join(root, "ai.sqlite"), mode: "create" }),
  );
  let attempts = 0;
  const host = unwrap(
    await createHost({
      credentialPersistence: fixturePersistence(store),
      workerRuntime,
      store,
      launchFences: store,
      delivery: null,
      resolve: async (_caller, options, namespace) => ({
        dispose: async () => {
          attempts++;
          if (attempts === 1) throw Error("fixture snapshot cleanup failure");
        },
        configuration: {
          namespace,
          provider: options.provider,
          config: options.config,

          workingDirectory: root,
          permissions: "tools_disabled",
        },
        artifact: new URL("./provider.mjs", import.meta.url).href,
      }),
    }),
  );
  t.after(async () => {
    await host.close(budget());
    await rm(root, { recursive: true, force: true });
  });
  unwrap(await store.saveConnection(caller, connection("one"), null));
  const session = unwrap(await host.createSession(caller, {}, budget()));
  unwrap(
    await host.submit(
      caller,
      {
        schemaVersion: 7,
        kind: "command",
        sessionId: session.namespace.sessionId,
        commandId: "quick",
        expiresAtMs: Date.now() + 60000,
        input: { type: "prompt", policy: "queue_next", text: "quick" },
      },
      budget(),
    ),
  );
  await until(
    async () =>
      unwrap(await store.command(session.namespace, "quick")).state ===
      "terminal",
  );
  assert.equal((await host.close(budget())).ok, false);
  assert.equal(attempts, 1);
  unwrap(await host.close(budget()));
  assert.equal(attempts, 2);
});

test("saving is independent; explicit testing records success and preserves inputs on rejection", async (t) => {
  const root = await mkdtemp(join(tmpdir(), "rss-connection-probe-"));
  const store = unwrap(
    openSqliteStore({ path: join(root, "ai.sqlite"), mode: "create" }),
  );
  let reject = false,
    disposed = 0;
  const diagnostics = [];
  const host = unwrap(
    await createHost({
      credentialPersistence: fixturePersistence(store),
      workerRuntime,
      store,
      launchFences: store,
      delivery: null,
      onDiagnostic: (diagnostic) => diagnostics.push(diagnostic),
      resolve: async (_caller, options, namespace) => {
        const artifact = new URL("./provider.mjs", import.meta.url);
        if (reject) artifact.searchParams.set("scenario", "probe_reject");
        return {
          dispose: async () => {
            assert.deepEqual(unwrap(await store.launches()), []);
            disposed++;
          },
          configuration: {
            namespace,
            provider: options.provider,
            config: options.config,

            workingDirectory: root,
            permissions: "tools_disabled",
          },
          artifact: artifact.href,
        };
      },
    }),
  );
  t.after(async () => {
    unwrap(await host.close(budget()));
    await rm(root, { recursive: true, force: true });
  });
  const first = unwrap(
    await host.saveConnection(
      caller,
      draftOf(connection("one")),
      null,
      budget(),
    ),
  );
  assert.equal(first.status, "unverified");
  assert.equal(disposed, 0);
  const ready = unwrap(await host.testConnection(caller, "one", 1, budget()));
  assert.equal(ready.status, "ready");
  assert.equal(ready.configRevision, 2);
  assert.equal(disposed, 1);
  const { readFile } = await import("node:fs/promises");
  const trace = (await readFile(join(root, "trace.ndjson"), "utf8"))
    .trim()
    .split("\n")
    .map(JSON.parse);
  assert.equal(trace.filter((row) => row.type === "dispatch").length, 1);
  assert.deepEqual(unwrap(await store.launches()), []);
  assert.deepEqual(
    unwrap(await host.listSessions(caller, { limit: 256 }, budget())).items,
    [],
  );
  assert.equal(
    unwrap(await host.deleteConnection(caller, "one", 2, budget())).status,
    "deleted",
  );
  const second = unwrap(
    await host.saveConnection(
      caller,
      draftOf(connection("two")),
      null,
      budget(),
    ),
  );
  reject = true;
  const failed = unwrap(await host.testConnection(caller, "two", 1, budget()));
  assert.equal(failed.lastTest.outcome, "failed");
  assert.deepEqual({ ...failed, lastTest: null }, second);
  assert.equal(
    unwrap(await host.connections(caller, budget())).preferences
      .defaultConnectionId,
    undefined,
  );
});

test("failed probe disposal leaves worker capacity available and retries cleanup on close", async (t) => {
  const root = await mkdtemp(join(tmpdir(), "rss-probe-disposal-"));
  const store = unwrap(
    openSqliteStore({ path: join(root, "ai.sqlite"), mode: "create" }),
  );
  let disposals = 0;
  const host = unwrap(
    await createHost({
      credentialPersistence: fixturePersistence(store),
      workerRuntime,
      store,
      launchFences: store,
      delivery: null,
      workerLimit: 1,
      resolve: async (_caller, options, namespace) => ({
        dispose: async () => {
          if (++disposals === 1) throw new Error("cleanup unavailable");
        },
        configuration: {
          namespace,
          provider: options.provider,
          config: options.config,
          workingDirectory: root,
          permissions: "tools_disabled",
        },
        artifact: new URL("./provider.mjs", import.meta.url).href,
      }),
    }),
  );
  unwrap(
    await host.saveConnection(
      caller,
      draftOf(connection("first")),
      null,
      budget(),
    ),
  );
  const first = unwrap(await host.testConnection(caller, "first", 1, budget()));
  assert.equal(first.lastTest.stage, "cleanup");
  unwrap(
    await host.saveConnection(
      caller,
      draftOf(connection("second")),
      null,
      budget(),
    ),
  );
  const second = unwrap(
    await host.testConnection(caller, "second", 1, budget()),
  );
  assert.equal(second.status, "ready");
  unwrap(await host.close(budget()));
  assert.ok(disposals >= 3);
  await rm(root, { recursive: true, force: true });
});

test("typed resolver and persistence failures keep closed codes and credential diagnostics", async (t) => {
  const root = await mkdtemp(join(tmpdir(), "rss-closed-errors-"));
  const store = unwrap(
    openSqliteStore({ path: join(root, "ai.sqlite"), mode: "create" }),
  );
  const diagnostics = [];
  let resolverFailure = true;
  const host = unwrap(
    await createHost({
      workerRuntime,
      store,
      launchFences: store,
      delivery: null,
      onDiagnostic: (value) => diagnostics.push(value),
      credentialPersistence: async () => {
        throw new HostFailure({
          code: "authentication_required",
          retry: "never",
        });
      },
      resolve: async (_caller, options, namespace) => {
        if (resolverFailure)
          throw new HostFailure({ code: "invalid_input", retry: "never" });
        return {
          configuration: {
            namespace,
            provider: options.provider,
            config: options.config,
            workingDirectory: root,
            permissions: "tools_disabled",
          },
          artifact: new URL("./provider.mjs", import.meta.url).href,
        };
      },
    }),
  );
  const candidate = draftOf(connection("typed"));
  assert.equal(
    (await host.saveConnection(caller, candidate, null, budget())).error.code,
    "authentication_required",
  );
  unwrap(await store.saveConnection(caller, connection("typed"), null));
  const tested = unwrap(
    await host.testConnection(caller, "typed", 1, budget()),
  );
  assert.equal(tested.lastTest.failure.code, "invalid_input");
  assert.equal(tested.lastTest.stage, "configuration");
  assert.deepEqual(diagnostics.at(-1), {
    stage: "credential",
    code: "authentication_required",
  });
  assert.doesNotMatch(JSON.stringify(diagnostics), /secret|path|model/i);
  unwrap(await host.close(budget()));
  await rm(root, { recursive: true, force: true });
});

test("top-level close attempts every independent owner and is retryable by its caller", async () => {
  const calls = [];
  await assert.rejects(
    closeOwners([
      async () => {
        calls.push("service");
        throw new Error("service failed");
      },
      async () => calls.push("execution"),
      async () => calls.push("host"),
      async () => calls.push("control"),
    ]),
    AggregateError,
  );
  assert.deepEqual(calls, ["service", "execution", "host", "control"]);
});

test("application persistence retains ciphertext without decrypting and lets only one competing revision commit", async (t) => {
  const root = await mkdtemp(join(tmpdir(), "rss-connection-persistence-"));
  const store = unwrap(
    openSqliteStore({ path: join(root, "ai.sqlite"), mode: "create" }),
  );
  const encrypted = new Uint8Array(32).fill(9);
  let available = true,
    rejectProbe = false;
  const host = unwrap(
    await createHost({
      workerRuntime,
      store,
      launchFences: store,
      delivery: null,
      callerAvailable: () => available,
      credentialPersistence: connectionPersistence(
        store,
        () => available,
        async () => true,
      ),
      resolve: async (_caller, options, namespace) => ({
        configuration: {
          namespace,
          provider: options.provider,
          config: options.config,
          workingDirectory: root,
          permissions: "tools_disabled",
        },
        artifact: new URL(
          rejectProbe
            ? "./provider.mjs?scenario=probe_reject"
            : "./provider.mjs",
          import.meta.url,
        ).href,
      }),
    }),
  );
  t.after(async () => {
    await host.close(budget());
    await rm(root, { recursive: true, force: true });
  });
  const first = unwrap(
    await host.saveNativeConnection(
      caller,
      draftOf(connection("one")),
      null,
      budget(),
      encrypted,
    ),
  );
  const second = unwrap(
    await host.saveConnection(
      caller,
      { ...draftOf(first), name: "edited" },
      1,
      budget(),
    ),
  );
  assert.deepEqual(
    unwrap(await store.encryptedSecret(caller, "one", second.configRevision)),
    encrypted,
  );
  const candidates = await Promise.all([
    host.saveConnection(
      caller,
      { ...draftOf(second), name: "winner-a" },
      2,
      budget(),
    ),
    host.saveConnection(
      caller,
      { ...draftOf(second), name: "winner-b" },
      2,
      budget(),
    ),
  ]);
  assert.equal(candidates.filter((result) => result.ok).length, 1);
  assert.equal(
    candidates.find((result) => !result.ok).error.code,
    "revision_conflict",
  );
  const current = unwrap(await store.connection(caller, "one"));
  assert.equal(current.configRevision, 3);
  assert.deepEqual(
    unwrap(await store.encryptedSecret(caller, "one", current.configRevision)),
    encrypted,
  );
  const ready = unwrap(await host.testConnection(caller, "one", 3, budget()));
  assert.equal(ready.status, "ready");
  assert.equal(ready.configRevision, 4);
  assert.deepEqual(
    unwrap(await store.encryptedSecret(caller, "one", 4)),
    encrypted,
  );
  rejectProbe = true;
  const failed = unwrap(await host.testConnection(caller, "one", 4, budget()));
  assert.equal(failed.lastTest.outcome, "failed");
  assert.equal(failed.configRevision, 4);
  assert.equal(failed.status, "ready");
  assert.deepEqual(
    unwrap(await store.encryptedSecret(caller, "one", 4)),
    encrypted,
  );
  available = false;
  assert.equal(
    (await host.saveConnection(caller, draftOf(current), 4, budget())).error
      .code,
    "unavailable",
  );
  assert.equal(unwrap(await store.connection(caller, "one")).configRevision, 4);
});

test("user fence settles persistent offline queues across pages and propagates durable failure", async (t) => {
  const { fixtureSession } = await import(
    "../../packages/ai-contract/dist/testing/index.js"
  );
  const root = await mkdtemp(join(tmpdir(), "rss-offline-fence-"));
  const path = join(root, "ai.sqlite");
  let store = unwrap(openSqliteStore({ path, mode: "create" }));
  const sessions = ["offline-one", "offline-two"].map((sessionId) => ({
    ...fixtureSession(),
    namespace: { ...caller, sessionId },
  }));
  for (const session of sessions) {
    unwrap(await store.create(session));
    const command = {
      schemaVersion: 7,
      kind: "command",
      sessionId: session.namespace.sessionId,
      commandId: "queued",
      expiresAtMs: Date.now() + 60000,
      input: {
        type: "prompt",
        policy: "queue_next",
        text: "never dispatch after switch",
      },
    };
    unwrap(
      await store.accept({
        namespace: session.namespace,
        command,
        expectedRevision: 0,
        expectedGeneration: activeStage(session).binding.generation,
        nowMs: Date.now(),
        retention: { retryWindowMs: 60000, receiptWindowMs: 86400000 },
        eventId: "accepted",
      }),
    );
  }
  unwrap(await store.close(budget()));
  store = unwrap(openSqliteStore({ path, mode: "open" }));
  let available = false,
    opens = 0,
    pages = 0;
  const list = store.listSessions.bind(store);
  store.listSessions = async (caller, query) => {
    pages++;
    return list(caller, { ...query, limit: 1 });
  };
  const host = unwrap(
    await createHost({
      credentialPersistence: fixturePersistence(store),
      workerRuntime,
      store,
      launchFences: store,
      delivery: null,
      callerAvailable: () => available,
      resolve: async () => {
        opens++;
        throw Error("must not open");
      },
    }),
  );
  t.after(async () => {
    unwrap(await host.close(budget()));
    await rm(root, { recursive: true, force: true });
  });
  available = true;
  const suspend = store.suspend.bind(store);
  store.suspend = async () => ({
    ok: false,
    error: { code: "unavailable", retry: "same_command" },
  });
  assert.equal((await host.suspendCaller(caller, budget())).ok, false);
  assert.equal(
    (await host.connections(caller, budget())).ok,
    true,
    "failed fence releases caller gate",
  );
  store.suspend = suspend;
  await suspendNativeCaller(host, {
    schemaVersion: 7,
    kind: "userContext",
    user: {
      schemaVersion: 7,
      kind: "testUser",
      userId: "alice",
      displayName: "Alice",
      nameKey: "alice",
    },
    generation: "restored-generation",
  });
  assert.ok(pages >= 3);
  assert.equal(opens, 0);
  assert.equal((await host.createSession(caller, {}, budget())).ok, false);
  for (const session of sessions) {
    const page = unwrap(
      await store.snapshotPage(session.namespace, { limit: 256 }),
    );
    const original = page.commands.find(
      (row) => row.command.commandId === "queued",
    );
    assert.equal(original.state, "cancelled");
    const cancel = page.commands.find(
      (row) => row.command.commandId === original.cancelledBy,
    );
    assert.equal(cancel.state, "acknowledged");
    assert.equal(cancel.acknowledgement.type, "queued_cancelled");
    assert.equal(page.session.status, "recovery_required");
    assert.equal(
      page.events.filter((event) => event.body.type === "terminal").length,
      0,
    );
    const revision = page.session.revision;
    host.activateCaller(caller);
    unwrap(await host.suspendCaller(caller, budget()));
    assert.equal(
      unwrap(await store.session(session.namespace)).revision,
      revision,
    );
  }
});

test("verification preserves definite failures and never calls unknown acceptance an authentication failure", async (t) => {
  for (const [scenario, expected] of [
    ["unknown", "unavailable"],
    ["empty_probe", "unavailable"],
    ["verification_auth", "authentication_required"],
    ["verification_model", "unsupported_capability"],
  ]) {
    const root = await mkdtemp(join(tmpdir(), "rss-verification-"));
    const store = unwrap(
      openSqliteStore({ path: join(root, "ai.sqlite"), mode: "create" }),
    );
    const host = unwrap(
      await createHost({
        credentialPersistence: fixturePersistence(store),
        workerRuntime,
        store,
        launchFences: store,
        delivery: null,
        resolve: async (_caller, options, namespace) => ({
          configuration: {
            namespace,
            provider: options.provider,
            config: options.config,
            workingDirectory: root,
            permissions: "tools_disabled",
          },
          artifact: new URL(
            `./provider.mjs?scenario=${scenario}`,
            import.meta.url,
          ).href,
        }),
      }),
    );
    try {
      unwrap(
        await host.saveConnection(
          caller,
          draftOf(connection("test")),
          null,
          budget(),
        ),
      );
      const result = unwrap(
        await host.testConnection(caller, "test", 1, budget()),
      );
      assert.equal(result.lastTest.outcome, "failed");
      assert.equal(result.lastTest.failure.code, expected);
      assert.equal((await store.connection(caller, "test")).ok, true);
    } finally {
      await host.close(budget());
      await rm(root, { recursive: true, force: true });
    }
  }
});

test("controlled connection verification requires the dedicated harmless tool call", async () => {
  for (const [toolWorks, empty] of [
    [false, false],
    [true, false],
    [true, true],
  ]) {
    const root = await mkdtemp(join(tmpdir(), "rss-tool-probe-"));
    const store = unwrap(
      openSqliteStore({ path: join(root, "ai.sqlite"), mode: "create" }),
    );
    const host = unwrap(
      await createHost({
        credentialPersistence: fixturePersistence(store),
        workerRuntime,
        store,
        launchFences: store,
        delivery: null,
        resolve: async (_caller, options, namespace) => ({
          configuration: {
            namespace,
            provider: options.provider,
            config: options.config,
            workingDirectory: root,
            permissions: "host_mediated",
          },
          artifact: new URL(
            `./provider.mjs?scenario=${empty ? "empty_tool_probe" : toolWorks ? "tool_probe" : "no_tool_probe"}`,
            import.meta.url,
          ).href,
          admission: {
            verifier: {
              verify: async () => ({
                ok: true,
                value: { platform: "fixture", verificationRef: "fixture-only" },
              }),
            },
          },
        }),
      }),
    );
    try {
      unwrap(
        await host.saveConnection(
          caller,
          { ...draftOf(connection("probe")), profile: "controlled_tools" },
          null,
          budget(),
        ),
      );
      const result = unwrap(
        await host.testConnection(caller, "probe", 1, budget()),
      );
      assert.equal(result.status === "ready", toolWorks && !empty);
      if (result.lastTest.outcome === "failed")
        assert.equal(
          result.lastTest.failure.code,
          empty ? "unavailable" : "unsupported_capability",
        );
      assert.equal((await store.connection(caller, "probe")).ok, true);
    } finally {
      await host.close(budget());
      await rm(root, { recursive: true, force: true });
    }
  }
});

test("enterprise and guest callers use native identity, never display name or provider identity", async () => {
  const user = {
    schemaVersion: 7,
    kind: "testUser",
    userId: "legacy",
    displayName: "same",
    nameKey: "same",
  };
  const context = {
    schemaVersion: 7,
    kind: "userContext",
    user,
    generation: "one",
  };
  assert.deepEqual(callerFor(context), {
    tenantId: "test-users",
    principalId: "legacy",
    authorityId: "desktop-fixture",
  });
  const identity = {
    mode: "enterprise",
    organizationId: "organization-a",
    tenantId: "tenant-a",
    principalId: "subject-a",
    authorityId: "instance-a",
    expiresAtMs: Date.now() + 1000,
  };
  const a = { ...context, identity };
  assert.equal(
    callerAvailable(a, callerFor(a), identity.expiresAtMs - 1),
    true,
  );
  assert.equal(callerAvailable(a, callerFor(a), identity.expiresAtMs), false);
  assert.equal(
    callerAvailable(
      { ...a, identity: { ...identity, expiresAtMs: 0 } },
      callerFor(a),
      1,
    ),
    false,
  );
  assert.throws(() => requireLocalExecution(a, callerFor(a)), /unbound origin/);
  assert.doesNotThrow(() => requireLocalExecution(context, callerFor(context)));
  const guest = {
    ...context,
    identity: {
      mode: "guest",
      authorityId: "desktop-guest",
      tenantId: "local-guest",
      principalId: "guest",
    },
  };
  assert.doesNotThrow(() => requireLocalExecution(guest, callerFor(guest)));
  assert.throws(
    () => requireLocalExecution(guest, callerFor(context)),
    /unbound origin/,
  );
  assert.deepEqual(callerFor(a), {
    tenantId: "tenant-a",
    principalId: "subject-a",
    authorityId: "instance-a",
  });
  for (const field of ["tenantId", "principalId", "authorityId"]) {
    const b = { ...a, identity: { ...identity, [field]: "other" } };
    assert.notDeepEqual(callerFor(a), callerFor(b));
  }
  let suspended;
  await suspendNativeCaller(
    {
      suspendCaller: async (caller) => {
        suspended = caller;
        return { ok: true, value: undefined };
      },
    },
    a,
  );
  assert.deepEqual(suspended, callerFor(a));
  await assert.rejects(() =>
    suspendNativeCaller(
      {
        suspendCaller: async () => {
          throw new Error("must not run");
        },
      },
      a,
      { ...a, generation: "new" },
    ),
  );
});

test("session creation replays a caller-owned identity without rebinding or duplicating", async (t) => {
  const root = await mkdtemp(join(tmpdir(), "rss-create-intent-"));
  const store = unwrap(
    openSqliteStore({ path: join(root, "ai.sqlite"), mode: "create" }),
  );
  const host = unwrap(
    await createHost({
      credentialPersistence: fixturePersistence(store),
      workerRuntime,
      store,
      launchFences: store,
      delivery: null,
      resolve: async () => {
        throw new Error("creation must not launch a provider");
      },
    }),
  );
  t.after(async () => {
    await host.close(budget());
    await rm(root, { recursive: true, force: true });
  });
  const intent = { sessionId: "first-send-identity" };
  const sessions = await Promise.all([
    host.createSession(caller, intent, budget()),
    host.createSession(caller, intent, budget()),
  ]);
  const first = unwrap(sessions[0]);
  assert.equal(first.namespace.sessionId, intent.sessionId);
  assert.deepEqual(unwrap(sessions[1]), first);
  const other = { ...caller, principalId: "bob" };
  assert.equal(
    unwrap(await host.createSession(other, intent, budget())).namespace
      .principalId,
    "bob",
  );
  assert.equal(
    unwrap(await host.listSessions(caller, { limit: 64 }, budget())).items
      .length,
    1,
  );
  assert.deepEqual(
    unwrap(
      await host.createSession(
        caller,
        { ...intent, connectionId: "not-a-rebind" },
        budget(),
      ),
    ),
    first,
  );
  assert.equal(
    (await host.createSession(caller, { sessionId: "" }, budget())).error.code,
    "invalid_input",
  );
});
