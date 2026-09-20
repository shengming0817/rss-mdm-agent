import test from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { openSqliteStore } from "../../packages/ai-store-sqlite/dist/index.js";
import { ConnectionSecrets } from "../../apps/ai-host/dist/secrets.js";
import { unwrap } from "../../packages/ai-contract/dist/testing/index.js";
const alice = { tenantId: "t", principalId: "alice", authorityId: "a" },
  bob = { ...alice, principalId: "bob" };
const row = (revision = 1) => ({
  schemaVersion: 5,
  kind: "connection",
  connectionId: "one",
  configRevision: revision,
  name: "Custom",
  provider: "codex",
  source: {
    type: "custom_api",
    apiUrl: "https://example.invalid",
    model: "chosen",
  },
  status: "ready",
  profile: "conversation",
});
const budget = { timeoutMs: 1000, signal: new AbortController().signal };
test("ciphertext is scoped by user/connection/revision and config/default/cipher delete atomically", async (t) => {
  const root = await mkdtemp(join(tmpdir(), "rss-encrypted-store-")),
    path = join(root, "ai.sqlite");
  const store = unwrap(openSqliteStore({ path, mode: "create" }));
  t.after(async () => {
    await store.close(budget);
    await rm(root, { recursive: true, force: true });
  });
  let calls = 0;
  const key = Buffer.alloc(32, 7);
  const secrets = new ConnectionSecrets(store, async (create) => {
    calls++;
    assert.equal(create, true);
    return key;
  });
  assert.equal(calls, 0); // Existing-config-only operation never calls the backend.
  const first = row(),
    plain = "sensitive-test-only-key",
    encrypted = await secrets.seal(alice, first, plain);
  assert.notDeepEqual(encrypted, await secrets.seal(alice, first, plain));
  unwrap(await store.saveConnection(alice, first, null, encrypted));
  assert.equal(await secrets.read(alice, first), plain);
  assert.equal(
    unwrap(await store.preferences(alice)).defaultConnectionId,
    "one",
  );
  assert.equal(
    JSON.stringify(unwrap(await store.connections(alice))).includes(plain),
    false,
  );
  assert.equal((await readFile(path)).includes(Buffer.from(plain)), false);
  unwrap(await store.saveConnection(bob, first, null, encrypted));
  await assert.rejects(secrets.read(bob, first));
  const second = row(2),
    sealed = await secrets.seal(
      alice,
      second,
      await secrets.read(alice, second, undefined, 1),
    );
  assert.equal(
    (await store.saveConnection(alice, second, 99, sealed)).error.code,
    "revision_conflict",
  );
  assert.deepEqual(unwrap(await store.connection(alice, "one")), first);
  unwrap(await store.saveConnection(alice, second, 1, sealed));
  assert.equal(await secrets.read(alice, second), plain);
  const tampered = row(3);
  unwrap(await store.saveConnection(alice, tampered, 2, encrypted));
  await assert.rejects(secrets.read(alice, tampered));
  const missing = new ConnectionSecrets(store, async (create) => {
    assert.equal(create, false);
    throw Error("missing key");
  });
  await assert.rejects(missing.read(alice, first));
  const deleted = { ...row(4), status: "deleted" };
  assert.equal(
    (await store.saveConnection(alice, deleted, 1)).error.code,
    "revision_conflict",
  );
  assert.equal(unwrap(await store.hasSecrets()), true);
  unwrap(await store.saveConnection(alice, deleted, 3));
  assert.equal(
    unwrap(await store.preferences(alice)).defaultConnectionId,
    undefined,
  );

  assert.equal(
    unwrap(await store.connection(alice, "one", 1)).configRevision,
    1,
  );
  await assert.rejects(secrets.read(alice, first));
  assert.equal(calls, 1);
  unwrap(await store.close(budget));
  const db = new DatabaseSync(path);
  assert.equal(
    db
      .prepare(
        "SELECT count(*) n FROM connections WHERE principal_id='alice' AND encrypted_secret IS NOT NULL",
      )
      .get().n,
    0,
  );
  db.close();
});
