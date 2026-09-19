import assert from "node:assert/strict";
import test from "node:test";
import { existsSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import {
  fixtureSession,
  fixtureCommand,
  acceptance,
  unwrap,
  emptyCommit,
  dispatchCommand,
  terminalCommit,
  seedSurface,
  surfaceCommit,
} from "../../packages/ai-contract/dist/testing/index.js";
import { deliveryFingerprint } from "../../packages/ai-contract/dist/index.js";
import { fixtureLimits } from "../../packages/ai-contract/dist/testing/index.js";
import { harness, budget } from "./support.mjs";

test("real SQLITE_FULL aborts acceptance without shrinking the deduplication history", async (t) => {
  const h = harness(t),
    store = unwrap(h.open()),
    initial = fixtureSession();
  unwrap(await store.create(initial));
  const db = h.raw(store),
    pages = db.prepare("PRAGMA page_count").get().page_count;
  db.exec(`PRAGMA max_page_count=${pages}`);
  const large = {
    ...fixtureCommand(),
    input: { type: "prompt", policy: "queue_next", text: "x".repeat(60000) },
  };
  const result = await store.accept(acceptance(initial, large));
  assert.equal(result.ok, false);
  assert.equal(result.error.code, "limit_exceeded");
  assert.equal(db.isTransaction, false);
  assert.equal(unwrap(await store.snapshot(initial.namespace, 1024)).cursor, 0);
  db.exec(`PRAGMA max_page_count=${pages + 128}`);
  unwrap(await store.accept(acceptance(initial, large)));
});
test("foreign key failure at the final write rolls back every earlier table", async (t) => {
  const h = harness(t),
    store = unwrap(h.open()),
    initial = fixtureSession();
  unwrap(await store.create(initial));
  const db = h.raw(store);
  db.exec(
    "CREATE TEMP TABLE fault_parent (id TEXT PRIMARY KEY); CREATE TEMP TABLE fault_child (id TEXT REFERENCES fault_parent(id))",
  );
  db.exec(
    "CREATE TEMP TRIGGER fault_fk BEFORE UPDATE ON main.sessions BEGIN INSERT INTO fault_child VALUES ('missing'); END",
  );
  assert.equal((await store.accept(acceptance(initial))).ok, false);
  assert.equal(
    unwrap(await store.snapshot(initial.namespace, 1024)).commands.length,
    0,
  );
  db.exec(
    "DROP TRIGGER fault_fk; DROP TABLE fault_child; DROP TABLE fault_parent",
  );
  unwrap(await store.accept(acceptance(initial)));
});
test("delivery is atomically tied to the event, content and stable operation identity", async (t) => {
  const h = harness(t),
    path = join(h.directory, "delivery.sqlite"),
    store = unwrap(h.open(path));
  const initial = fixtureSession();
  unwrap(await store.create(initial));
  unwrap(await store.accept(acceptance(initial)));
  const head = unwrap(await store.session(initial.namespace)),
    event = unwrap(await store.events(initial.namespace, 0, 1))[0];
  const delivery = {
    schemaVersion: 2,
    kind: "delivery",
    namespace: initial.namespace,
    operationId: "operation-1",
    eventId: event.eventId,
    target: "service-1",
    contentHash: deliveryFingerprint(event, "service-1", fixtureLimits),
    status: "pending",
    attempts: 0,
    nextAttemptAtMs: 0,
    retry: "receiver_idempotent",
  };
  unwrap(await store.commit({ ...emptyCommit(head), deliveries: [delivery] }));
  let next = unwrap(await store.session(initial.namespace));
  assert.equal(
    (
      await store.commit({
        ...emptyCommit(next),
        deliveries: [{ ...delivery, target: "other" }],
      })
    ).ok,
    false,
  );
  assert.deepEqual(unwrap(await store.deliveries(10, 0)).items, [delivery]);
  unwrap(
    await store.commit({
      ...emptyCommit(next),
      deliveries: [{ ...delivery, attempts: 1, status: "delivered" }],
    }),
  );
  next = unwrap(await store.session(initial.namespace));
  unwrap(
    await store.commit({
      ...emptyCommit(next),
      deliveries: [{ ...delivery, attempts: 1, status: "delivered" }],
    }),
  );
  assert.deepEqual(unwrap(await store.deliveries(10, 0)).items, []);
  unwrap(await store.close(budget()));
  const reopened = unwrap(h.open(path, "open"));
  assert.deepEqual(unwrap(await reopened.deliveries(10, 0)).items, []);
});
test("snapshot cursor bridges surface create/update/delete and restart without gaps", async (t) => {
  const h = harness(t),
    path = join(h.directory, "surface.sqlite"),
    store = unwrap(h.open(path));
  const seeded = await seedSurface(store),
    before = unwrap(await store.snapshot(seeded.session.namespace, 1024));
  const updated = { ...seeded.surface, revision: 1 };
  unwrap(
    await store.commit(
      surfaceCommit(before.session, updated, seeded.interaction),
    ),
  );
  const head = unwrap(await store.session(before.session.namespace));
  unwrap(
    await store.commit(
      surfaceCommit(
        head,
        { ...updated, revision: 2, status: "deleted" },
        seeded.interaction,
      ),
    ),
  );
  const delta = unwrap(await store.events(head.namespace, before.cursor, 1024));
  assert.deepEqual(
    delta.map((e) => e.sequence),
    [before.cursor + 1, before.cursor + 2, before.cursor + 3],
  );
  assert.deepEqual(
    delta.filter((e) => e.body.type === "surface").map((e) => e.body.operation),
    ["update", "delete"],
  );
  const full = unwrap(await store.snapshot(head.namespace, 1024));
  assert.deepEqual([...before.events, ...delta], full.events);
  unwrap(await store.close(budget()));
  assert.deepEqual(
    unwrap(await unwrap(h.open(path, "open")).snapshot(head.namespace, 1024)),
    full,
  );
});
test("schema creation is transactional and partial/newer/foreign files are not recreated", async (t) => {
  const h = harness(t),
    path = join(h.directory, "migration.sqlite"),
    exec = DatabaseSync.prototype.exec;
  DatabaseSync.prototype.exec = function (sql) {
    if (sql.startsWith("CREATE TABLE surfaces"))
      throw new Error("test migration interruption");
    return exec.call(this, sql);
  };
  try {
    assert.equal(h.open(path).ok, false);
  } finally {
    DatabaseSync.prototype.exec = exec;
  }
  const raw = new DatabaseSync(path);
  assert.equal(raw.prepare("PRAGMA user_version").get().user_version, 0);
  assert.equal(
    raw.prepare("SELECT count(*) AS n FROM sqlite_schema").get().n,
    0,
  );
  raw.close();
  assert.equal(h.open(path, "open").ok, false);
  const newer = join(h.directory, "newer.sqlite"),
    store = unwrap(h.open(newer));
  unwrap(await store.close(budget()));
  const future = new DatabaseSync(newer);
  future.exec("PRAGMA user_version=2");
  future.close();
  const bytes = readFileSync(newer);
  assert.equal(h.open(newer, "open").error.code, "unsupported_version");
  assert.deepEqual(readFileSync(newer), bytes);
  const foreign = join(h.directory, "foreign.sqlite");
  writeFileSync(foreign, "not a database", { mode: 0o600 });
  assert.equal(h.open(foreign, "open").ok, false);
  assert.equal(readFileSync(foreign, "utf8"), "not a database");
  const missing = join(h.directory, "missing.sqlite");
  assert.equal(h.open(missing, "open").ok, false);
  assert.equal(existsSync(missing), false);
});
test("missing tables refuse open", async (t) => {
  const h = harness(t),
    path = join(h.directory, "broken.sqlite"),
    store = unwrap(h.open(path));
  unwrap(await store.close(budget()));
  const raw = new DatabaseSync(path);
  raw.exec("DROP TABLE deliveries");
  raw.close();
  assert.equal(h.open(path, "open").error.code, "unsupported_version");
});
test("capacity fails explicitly; retirement retains an irreversible namespace tombstone", async (t) => {
  const h = harness(t),
    path = join(h.directory, "retire.sqlite"),
    store = unwrap(h.open(path));
  const initial = fixtureSession();
  unwrap(await store.create(initial));
  unwrap(await store.accept(acceptance(initial)));
  const started = await dispatchCommand(
    store,
    unwrap(await store.session(initial.namespace)),
  );
  unwrap(await store.commit(terminalCommit(started.session, started.record)));
  const head = unwrap(await store.session(initial.namespace));
  unwrap(
    await store.retire(head.namespace, head.revision, head.binding.generation),
  );
  assert.equal(unwrap(await store.pruneRetired(200)), 0);
  assert.equal(unwrap(await store.pruneRetired(201)), 1);
  unwrap(await store.close(budget()));
  const reopened = unwrap(h.open(path, "open"));
  assert.equal((await reopened.create(initial)).error.code, "content_conflict");
  const limited = unwrap(h.open(undefined, "create", { maxSessionRecords: 2 }));
  unwrap(await limited.create(initial));
  assert.equal(
    (await limited.accept(acceptance(initial))).error.code,
    "limit_exceeded",
  );
  assert.equal(
    unwrap(await limited.snapshot(initial.namespace, 1024)).cursor,
    0,
  );
});
test("WAL FULL, disabled extensions and private files are the effective runtime settings", async (t) => {
  const h = harness(t),
    path = join(h.directory, "settings.sqlite"),
    store = unwrap(h.open(path)),
    db = h.raw(store);
  assert.equal(db.prepare("PRAGMA journal_mode").get().journal_mode, "wal");
  assert.equal(
    db.prepare("PRAGMA locking_mode").get().locking_mode,
    "exclusive",
  );
  assert.equal(db.prepare("PRAGMA synchronous").get().synchronous, 2);
  assert.equal(db.prepare("PRAGMA foreign_keys").get().foreign_keys, 1);
  assert.equal(db.prepare("PRAGMA trusted_schema").get().trusted_schema, 0);
  assert.throws(() => db.loadExtension("not-allowed"));
  if (process.platform !== "win32")
    for (const file of [path, path + "-wal"])
      if (existsSync(file)) assert.equal(statSync(file).mode & 0o077, 0);
  assert.equal(existsSync(path + "-shm"), false);
});

test("orphaned rows fail integrity checks without rewriting the database", async (t) => {
  const h = harness(t),
    path = join(h.directory, "orphan.sqlite"),
    store = unwrap(h.open(path));
  const initial = fixtureSession();
  unwrap(await store.create(initial));
  unwrap(await store.accept(acceptance(initial)));
  unwrap(await store.close(budget()));
  const raw = new DatabaseSync(path);
  raw.exec("PRAGMA foreign_keys=OFF; DELETE FROM commands");
  raw.close();
  assert.equal(h.open(path, "open").error.code, "unsupported_version");
});
test("query byte budgets and invalid cursors fail with value-free errors", async (t) => {
  const h = harness(t),
    store = unwrap(h.open(undefined, "create", { maxQueryBytes: 1024 }));
  const initial = fixtureSession();
  unwrap(await store.create(initial));
  const large = {
    ...fixtureCommand(),
    input: {
      type: "prompt",
      policy: "queue_next",
      text: "private-prompt-".repeat(400),
    },
  };
  unwrap(await store.accept(acceptance(initial, large)));
  const result = await store.recovery(10);
  assert.equal(result.error.code, "limit_exceeded");
  assert.equal(JSON.stringify(result).includes("private-prompt"), false);
  assert.equal(
    (await store.recovery(10, "private-invalid-cursor")).error.code,
    "invalid_input",
  );
});

test("refusing a foreign SQLite file preserves its bytes and journal mode", async (t) => {
  const h = harness(t),
    path = join(h.directory, "foreign-valid.sqlite");
  writeFileSync(path, "", { mode: 0o600 });
  const raw = new DatabaseSync(path);
  raw.exec(
    "CREATE TABLE foreign_data (body TEXT); INSERT INTO foreign_data VALUES ('keep-me')",
  );
  assert.equal(raw.prepare("PRAGMA journal_mode").get().journal_mode, "delete");
  raw.close();
  const bytes = readFileSync(path);
  assert.equal(h.open(path, "open").error.code, "unsupported_version");
  assert.deepEqual(readFileSync(path), bytes);
  for (const suffix of ["-wal", "-shm", "-journal"])
    assert.equal(existsSync(path + suffix), false);
  const check = new DatabaseSync(path, { readOnly: true });
  try {
    assert.equal(
      check.prepare("PRAGMA journal_mode").get().journal_mode,
      "delete",
    );
  } finally {
    check.close();
  }
});
