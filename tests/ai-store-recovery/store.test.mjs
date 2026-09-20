import { activeStage } from "../../packages/ai-contract/dist/index.js";
import { readSnapshot } from "../../packages/ai-contract/dist/testing/index.js";
import assert from "node:assert/strict";
import test from "node:test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import {
  fixtureSession,
  acceptance,
  unwrap,
  runStoreConformance,
  restoredSession,
} from "../../packages/ai-contract/dist/testing/index.js";

const budget = () => ({
  timeoutMs: 1000,
  signal: new AbortController().signal,
});
async function harness(t) {
  const { openSqliteStore } = await import(
    "../../packages/ai-store-sqlite/dist/index.js"
  );
  const directory = mkdtempSync(join(tmpdir(), "rss-ai-store-")),
    stores = [];
  t.after(async () => {
    for (const s of stores) await s.close(budget());
    rmSync(directory, { recursive: true, force: true });
  });
  let next = 0;
  return {
    directory,
    open(
      path = join(directory, `store-${next++}.sqlite`),
      mode = "create",
      options = {},
    ) {
      const result = openSqliteStore({ path, mode, ...options });
      if (result.ok) stores.push(result.value);
      return result;
    },
  };
}
test("real SQLite passes the shared contract including verified recovery", async (t) => {
  const h = await harness(t);
  await runStoreConformance(() => unwrap(h.open()));
});
test("reopen preserves acceptance but requires a verified new generation before mutation", async (t) => {
  const h = await harness(t),
    path = join(h.directory, "restart.sqlite");
  let store = unwrap(h.open(path));
  const initial = fixtureSession();
  activeStage(initial).capabilities.continuation = "across_processes";
  unwrap(await store.create(initial));
  const input = acceptance(initial),
    receipt = unwrap(await store.accept(input));
  const before = unwrap(await readSnapshot(store, initial.namespace));
  unwrap(await store.close(budget()));
  store = unwrap(h.open(path, "open"));
  assert.deepEqual(unwrap(await store.accept(input)), receipt);
  assert.deepEqual(
    unwrap(await readSnapshot(store, initial.namespace)),
    before,
  );
  const another = acceptance(before.session, {
    ...input.command,
    commandId: "other",
  });
  assert.equal((await store.accept(another)).error.code, "stale_binding");
  const restored = await restoredSession(before.session, "restart-generation");
  const next = unwrap(
    await store.rebind({
      namespace: initial.namespace,
      expectedRevision: before.session.revision,
      expectedGeneration: activeStage(initial).binding.generation,
      restored,
      eventId: "restart",
    }),
  );
  unwrap(await store.accept(acceptance(next, another.command)));
});
test("failure at the final session CAS rolls back command, receipt and event", async (t) => {
  const h = await harness(t);
  const original = DatabaseSync.prototype.prepare;
  let connection;
  DatabaseSync.prototype.prepare = function (...args) {
    connection = this;
    return original.apply(this, args);
  };
  let store;
  try {
    store = unwrap(h.open());
  } finally {
    DatabaseSync.prototype.prepare = original;
  }
  const initial = fixtureSession();
  unwrap(await store.create(initial));
  connection.exec(
    "CREATE TEMP TRIGGER reject_session BEFORE UPDATE ON sessions BEGIN SELECT RAISE(ABORT, 'test-only'); END",
  );
  const result = await store.accept(acceptance(initial));
  assert.equal(result.ok, false);
  assert.deepEqual(unwrap(await readSnapshot(store, initial.namespace)), {
    session: initial,
    cursor: 0,
    events: [],
    commands: [],
    interactions: [],
    surfaces: [],
  });
  connection.exec("DROP TRIGGER reject_session");
  unwrap(await store.accept(acceptance(initial)));
});
