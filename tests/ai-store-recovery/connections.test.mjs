import assert from "node:assert/strict";
import test from "node:test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { openSqliteStore } from "../../packages/ai-store-sqlite/dist/index.js";
import { productSession } from "../../packages/ai-contract/dist/index.js";
import { unwrap } from "../../packages/ai-contract/dist/testing/index.js";
const a = { tenantId: "test", principalId: "alice", authorityId: "desktop" };
const b = { ...a, principalId: "bob" };
const connection = (id) => ({
  schemaVersion: 5,
  kind: "connection",
  connectionId: id,
  name: id,
  provider: "deepseek",
  configRevision: 1,
  credentialRevision: 1,
  accountRef: `account-${id}`,
  profile: "conversation",
  status: "ready",
  source: {
    type: "custom_api",
    apiUrl: "https://api.deepseek.com",
    model: "deepseek-chat",
  },
  credentialRef: `credential-${id}`,
});
test("empty sessions and per-user connection revisions persist without provider credentials", async (t) => {
  const root = mkdtempSync(join(tmpdir(), "rss-connections-")),
    path = join(root, "state.sqlite");
  let store = unwrap(openSqliteStore({ path, mode: "create" }));
  const budget = { timeoutMs: 1000, signal: new AbortController().signal };
  t.after(async () => {
    await store.close(budget);
    rmSync(root, { recursive: true, force: true });
  });
  const session = productSession({ ...a, sessionId: "empty" });
  unwrap(await store.create(session));
  assert.deepEqual(unwrap(await store.session(session.namespace)), session);
  const first = connection("first"),
    second = connection("second");
  unwrap(await store.saveConnection(a, first, null));
  unwrap(await store.saveConnection(a, second, null));
  assert.equal(unwrap(await store.preferences(a)).defaultConnectionId, "first");
  assert.deepEqual(unwrap(await store.connections(b)), []);
  assert.equal((await store.connection(b, "first")).ok, false);
  assert.equal(
    (await store.selectConnection({ ...b, sessionId: "empty" }, "first", 0)).ok,
    false,
  );
  unwrap(await store.selectConnection(session.namespace, "second", 0));
  const deleted = { ...first, status: "deleted", configRevision: 2 };
  unwrap(await store.saveConnection(a, deleted, 1));
  assert.equal(
    unwrap(await store.preferences(a)).defaultConnectionId,
    undefined,
  );
  assert.deepEqual(unwrap(await store.connection(a, "first", 1)), first);
  assert.equal(
    (await store.saveConnection(a, { ...second, configRevision: 3 }, 1)).ok,
    false,
  );
  unwrap(await store.close(budget));
  store = unwrap(openSqliteStore({ path, mode: "open" }));
  assert.equal(
    unwrap(await store.session(session.namespace)).selectedConnectionId,
    "second",
  );
  assert.deepEqual(unwrap(await store.connection(a, "first", 1)), first);
});
