import test from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { openSqliteStore } from "../../packages/ai-store-sqlite/dist/index.js";
import {
  ConnectionSecrets,
  connectionPersistence,
} from "../../apps/ai-host/dist/secrets.js";
import { unwrap } from "../../packages/ai-contract/dist/testing/index.js";
const caller = { tenantId: "t", principalId: "alice", authorityId: "a" };
const budget = { timeoutMs: 1000, signal: new AbortController().signal };
const row = (revision = 1) => ({
  schemaVersion: 6,
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
  status: "unverified",
  profile: "conversation",
  lastTest: null,
});
test("native ciphertext persists atomically, is retained without decryption and is cleared from every deleted revision", async (t) => {
  const root = await mkdtemp(join(tmpdir(), "rss-ciphertext-")),
    path = join(root, "ai.sqlite");
  let store = unwrap(openSqliteStore({ path, mode: "create" }));
  t.after(async () => {
    await store.close(budget);
    await rm(root, { recursive: true, force: true });
  });
  const ciphertext = new Uint8Array(32).fill(9);
  const persist = connectionPersistence(store, () => true);
  const first = unwrap(await persist(caller, row(), null, ciphertext, budget));
  assert.equal(
    (await persist(caller, row(2), 99, new Uint8Array(32).fill(8), budget))
      .error.code,
    "revision_conflict",
  );
  assert.deepEqual(
    unwrap(await store.encryptedSecret(caller, "one", 1)),
    ciphertext,
  );
  const second = unwrap(
    await persist(
      caller,
      {
        ...row(2),
        name: "Edited",
        source: { ...row().source, model: "other" },
      },
      1,
      undefined,
      budget,
    ),
  );
  assert.deepEqual(
    unwrap(await store.encryptedSecret(caller, "one", 2)),
    ciphertext,
  );
  for (const change of [
    { provider: "claude" },
    { source: { ...second.source, apiUrl: "https://other.example" } },
    { source: { ...second.source, credentialType: "auth_token" } },
  ]) {
    assert.equal(
      (
        await persist(
          caller,
          { ...second, ...change, configRevision: 3 },
          2,
          undefined,
          budget,
        )
      ).error.code,
      "authentication_required",
    );
  }
  assert.equal(
    (await store.connection({ ...caller, principalId: "bob" }, "one")).ok,
    false,
  );
  let decrypts = 0;
  const secrets = new ConnectionSecrets(store, async (owner, bytes) => {
    decrypts++;
    assert.equal(owner.principalId, "alice");
    assert.equal(owner.endpoint, "https://example.invalid/");
    assert.equal("configRevision" in owner, false);
    assert.equal(Buffer.from(bytes).equals(ciphertext), true);
    return "synthetic-activation-only";
  });
  assert.equal(decrypts, 0);
  assert.equal((await secrets.read(caller, second)).length > 0, true);
  assert.equal(decrypts, 1);
  assert.equal(
    JSON.stringify(unwrap(await store.connections(caller))).includes(
      "synthetic-activation-only",
    ),
    false,
  );
  assert.equal(
    (await readFile(path)).includes(Buffer.from("synthetic-activation-only")),
    false,
  );
  unwrap(await store.close(budget));
  store = unwrap(openSqliteStore({ path, mode: "open" }));
  assert.deepEqual(unwrap(await store.connection(caller, "one")), second);
  assert.deepEqual(
    unwrap(await store.encryptedSecret(caller, "one", 2)),
    ciphertext,
  );
  const currentPersistence = connectionPersistence(store, () => true);
  unwrap(
    await currentPersistence(
      caller,
      { ...second, configRevision: 3, status: "ready" },
      2,
      undefined,
      budget,
    ),
  );
  assert.equal(
    unwrap(await store.preferences(caller)).defaultConnectionId,
    "one",
  );
  unwrap(
    await currentPersistence(
      caller,
      { ...second, configRevision: 4 },
      3,
      undefined,
      budget,
    ),
  );
  assert.equal(
    unwrap(await store.preferences(caller)).defaultConnectionId,
    undefined,
  );
  unwrap(
    await currentPersistence(
      caller,
      { ...second, configRevision: 5, status: "deleted" },
      4,
      undefined,
      budget,
    ),
  );
  assert.equal(unwrap(await store.hasSecrets()), false);
  assert.equal((await store.encryptedSecret(caller, "one", 1)).ok, false);
  unwrap(await store.close(budget));
  const db = new DatabaseSync(path);
  assert.equal(
    db
      .prepare(
        "SELECT count(*) n FROM connections WHERE encrypted_secret IS NOT NULL",
      )
      .get().n,
    0,
  );
  db.close();
});
