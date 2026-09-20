import assert from "node:assert/strict";
import test from "node:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createHost } from "../../packages/ai-host/dist/index.js";
import { openSqliteStore } from "../../packages/ai-store-sqlite/dist/index.js";
import { activeStage } from "../../packages/ai-contract/dist/index.js";
const caller = {
  tenantId: "test",
  principalId: "alice",
  authorityId: "desktop",
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
  credentialRevision: 1,
  accountRef: `account-${id}`,
  profile: "conversation",
  status: "ready",
  source: {
    type: "custom_api",
    apiUrl: "https://example.invalid/v1",
    model: "test",
  },
  credentialRef: `credential-${id}`,
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
  let opened = 0;
  const host = unwrap(
    await createHost({
      store,
      launchFences: store,
      delivery: null,
      resolve: async (_caller, options, namespace) => {
        opened++;
        return {
          configuration: {
            namespace,
            provider: options.provider,
            config: options.config,
            accountRef: options.accountRef,
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
});

test("switching test users cancels queued model work and keeps the old user's receipts scoped", async (t) => {
  const root = await mkdtemp(join(tmpdir(), "rss-switch-user-"));
  const store = unwrap(
    openSqliteStore({ path: join(root, "ai.sqlite"), mode: "create" }),
  );
  const host = unwrap(
    await createHost({
      store,
      launchFences: store,
      delivery: null,
      resolve: async (_caller, options, namespace) => ({
        configuration: {
          namespace,
          provider: options.provider,
          config: options.config,
          accountRef: options.accountRef,
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
  unwrap(await host.submit(caller, queued, budget()));
  unwrap(await host.suspendCaller(caller, budget()));
  assert.equal(
    unwrap(await store.command(session.namespace, "queued")).state,
    "cancelled",
  );
  const bob = { ...caller, principalId: "bob" };
  assert.equal((await host.submit(bob, running, budget())).ok, false);
  assert.deepEqual(
    unwrap(await host.connections(bob, budget())).connections,
    [],
  );
  assert.equal(
    (await host.submit(caller, command("late", "quick"), budget())).ok,
    false,
  );
  host.activateCaller(caller);
  assert.deepEqual(
    unwrap(await host.submit(caller, running, budget())),
    receipt,
  );
});

test("saving a connection requires a completed model probe and preserves the previous revision on rejection", async (t) => {
  const root = await mkdtemp(join(tmpdir(), "rss-connection-probe-"));
  const store = unwrap(
    openSqliteStore({ path: join(root, "ai.sqlite"), mode: "create" }),
  );
  let reject = false;
  const host = unwrap(
    await createHost({
      store,
      launchFences: store,
      delivery: null,
      resolve: async (_caller, options, namespace) => {
        if (reject) throw Error("fixture authentication rejected");
        return {
          configuration: {
            namespace,
            provider: options.provider,
            config: options.config,
            accountRef: options.accountRef,
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
  const first = unwrap(
    await host.saveConnection(
      caller,
      { ...connection("one"), status: "unverified" },
      null,
      budget(),
    ),
  );
  assert.equal(first.status, "ready");
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
  reject = true;
  assert.equal(
    (
      await host.saveConnection(
        caller,
        { ...first, name: "rejected", configRevision: 2 },
        1,
        budget(),
      )
    ).ok,
    false,
  );
  assert.deepEqual(unwrap(await store.connection(caller, "one")), first);
  assert.equal(
    unwrap(await host.connections(caller, budget())).preferences
      .defaultConnectionId,
    "one",
  );
});
