import { workerRuntime } from "./worker-runtime.mjs";
import assert from "node:assert/strict";
import test from "node:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createHost, HostFailure } from "../../packages/ai-host/dist/index.js";
import { openSqliteStore } from "../../packages/ai-store-sqlite/dist/index.js";
import { activeStage } from "../../packages/ai-contract/dist/index.js";
import {
  connectionPersistence,
  ConnectionSecrets,
} from "../../apps/ai-host/dist/secrets.js";
import {
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
  schemaVersion: 5,
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
    schemaVersion: 5,
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
  unwrap(
    await host.saveConnection(
      caller,
      { ...connection("two"), configRevision: 2, status: "deleted" },
      1,
      budget(),
    ),
  );
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
    schemaVersion: 5,
    kind: "userContext",
    user: {
      schemaVersion: 5,
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
    schemaVersion: 5,
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
        schemaVersion: 5,
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

test("saving a connection requires a completed model probe and preserves the previous revision on rejection", async (t) => {
  const root = await mkdtemp(join(tmpdir(), "rss-connection-probe-"));
  const store = unwrap(
    openSqliteStore({ path: join(root, "ai.sqlite"), mode: "create" }),
  );
  let reject = false,
    disposed = 0;
  const diagnostics = [];
  const host = unwrap(
    await createHost({
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
      { ...connection("one"), status: "unverified" },
      null,
      budget(),
    ),
  );
  assert.equal(first.status, "ready");
  assert.equal(
    disposed,
    1,
    "probe artifacts are disposed only after the worker stopped",
  );
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
  const deleted = unwrap(
    await host.saveConnection(
      caller,
      { ...first, configRevision: 2, status: "deleted" },
      1,
      budget(),
    ),
  );
  assert.equal(deleted.status, "deleted");
  // Use a separate ready connection to verify a failed probe retains the previous revision.
  const second = unwrap(
    await host.saveConnection(
      caller,
      { ...connection("two"), status: "unverified" },
      null,
      budget(),
    ),
  );
  reject = true;
  assert.equal(
    (
      await host.saveConnection(
        caller,
        { ...second, name: "rejected", configRevision: 2 },
        1,
        budget(),
      )
    ).ok,
    false,
  );
  assert.deepEqual(unwrap(await store.connection(caller, "two")), second);
  assert.equal(
    unwrap(await host.connections(caller, budget())).preferences
      .defaultConnectionId,
    "two",
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
  const first = await host.saveConnection(
    caller,
    { ...connection("first"), status: "unverified" },
    null,
    budget(),
  );
  assert.equal(first.ok, true);
  const second = await host.saveConnection(
    caller,
    { ...connection("second"), status: "unverified" },
    null,
    budget(),
  );
  assert.equal(second.ok, true, JSON.stringify(second));
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
      persistConnection: async () => {
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
  const candidate = { ...connection("typed"), status: "unverified" };
  assert.equal(
    (await host.saveConnection(caller, candidate, null, budget())).error.code,
    "invalid_input",
  );
  resolverFailure = false;
  assert.equal(
    (await host.saveConnection(caller, candidate, null, budget())).error.code,
    "authentication_required",
  );
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

test("application persistence reencrypts retained secrets and lets only one competing revision commit", async (t) => {
  const root = await mkdtemp(join(tmpdir(), "rss-connection-persistence-"));
  const store = unwrap(
    openSqliteStore({ path: join(root, "ai.sqlite"), mode: "create" }),
  );
  const secrets = new ConnectionSecrets(store, async () => Buffer.alloc(32, 9));
  let available = true;
  const host = unwrap(
    await createHost({
      workerRuntime,
      store,
      launchFences: store,
      delivery: null,
      callerAvailable: () => available,
      persistConnection: connectionPersistence(store, secrets, () => available),
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
    await host.close(budget());
    await rm(root, { recursive: true, force: true });
  });
  const first = unwrap(
    await host.saveConnection(
      caller,
      { ...connection("one"), status: "unverified" },
      null,
      budget(),
      "retained-secret",
    ),
  );
  const second = unwrap(
    await host.saveConnection(
      caller,
      { ...first, name: "edited", configRevision: 2, status: "unverified" },
      1,
      budget(),
    ),
  );
  assert.equal(await secrets.read(caller, second), "retained-secret");
  const candidates = await Promise.all([
    host.saveConnection(
      caller,
      { ...second, name: "winner-a", configRevision: 3, status: "unverified" },
      2,
      budget(),
    ),
    host.saveConnection(
      caller,
      { ...second, name: "winner-b", configRevision: 3, status: "unverified" },
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
  assert.equal(await secrets.read(caller, current), "retained-secret");
  available = false;
  assert.equal(
    (
      await host.saveConnection(
        caller,
        { ...current, configRevision: 4, status: "unverified" },
        3,
        budget(),
      )
    ).error.code,
    "unavailable",
  );
  assert.equal(unwrap(await store.connection(caller, "one")).configRevision, 3);
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
      schemaVersion: 5,
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
    schemaVersion: 5,
    kind: "userContext",
    user: {
      schemaVersion: 5,
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
      const result = await host.saveConnection(
        caller,
        connection("test"),
        null,
        budget(),
      );
      assert.equal(result.ok, false);
      assert.equal(result.error.code, expected);
      assert.equal((await store.connection(caller, "test")).ok, false);
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
      const result = await host.saveConnection(
        caller,
        { ...connection("probe"), profile: "controlled_tools" },
        null,
        budget(),
      );
      assert.equal(result.ok, toolWorks && !empty);
      if (!result.ok)
        assert.equal(
          result.error.code,
          empty ? "unavailable" : "unsupported_capability",
        );
      assert.equal(
        (await store.connection(caller, "probe")).ok,
        toolWorks && !empty,
      );
    } finally {
      await host.close(budget());
      await rm(root, { recursive: true, force: true });
    }
  }
});
