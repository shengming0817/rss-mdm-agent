import assert from "node:assert/strict";
import test from "node:test";
import { writeFileSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import {
  fixtureSession,
  unwrap,
} from "../../packages/ai-contract/dist/testing/index.js";
import { harness, budget } from "./support.mjs";

test("runtime declaration and CI preflight reject an unverified Node version before work", () => {
  const root = JSON.parse(
    readFileSync(new URL("../../package.json", import.meta.url)),
  );
  const adapter = JSON.parse(
    readFileSync(
      new URL("../../packages/ai-store-sqlite/package.json", import.meta.url),
    ),
  );
  assert.equal(root.engines.node, adapter.engines.node);
  const result = spawnSync(
    process.execPath,
    [
      "--input-type=module",
      "-e",
      `Object.defineProperty(process.versions,"node",{value:"24.0.0"});await import(${JSON.stringify(new URL("../../scripts/ci.mjs", import.meta.url).href)});`,
    ],
    { timeout: 5000, encoding: "utf8" },
  );
  assert.equal(result.status, 1);
  assert.match(result.stderr, /Node version/);
  assert.equal(result.stdout, "");
});
test("cancelled close stops admission but retains ownership until fresh cleanup", async (t) => {
  const h = harness(t),
    path = join(h.directory, "closing.sqlite"),
    store = unwrap(h.open(path));
  unwrap(await store.create(fixtureSession()));
  const control = new AbortController();
  control.abort();
  assert.deepEqual(await store.close({ ...budget(), signal: control.signal }), {
    ok: false,
    error: { code: "unavailable", retry: "same_command" },
  });
  assert.deepEqual(await store.session(fixtureSession().namespace), {
    ok: false,
    error: { code: "unavailable", retry: "never" },
  });
  assert.deepEqual(h.open(path, "open", { busyTimeoutMs: 50 }), {
    ok: false,
    error: { code: "unavailable", retry: "same_command" },
  });
  unwrap(await store.close(budget()));
  unwrap(await store.close(budget()));
  unwrap(h.open(path, "open"));
});
test("failed close preserves cleanup retry and invalid budgets never release ownership", async (t) => {
  const h = harness(t),
    path = join(h.directory, "close-retry.sqlite"),
    store = unwrap(h.open(path));
  unwrap(await store.create(fixtureSession()));
  assert.equal(
    (await store.close({ ...budget(), timeoutMs: 0 })).error.code,
    "invalid_input",
  );
  const db = h.raw(store),
    close = db.close.bind(db);
  db.close = () => {
    throw Object.assign(new Error("private-path SQL"), { errcode: 5 });
  };
  assert.deepEqual(await store.close(budget()), {
    ok: false,
    error: { code: "unavailable", retry: "same_command" },
  });
  assert.equal((await store.session(fixtureSession().namespace)).ok, false);
  assert.equal(h.open(path, "open", { busyTimeoutMs: 50 }).ok, false);
  db.close = close;
  unwrap(await store.close(budget()));
  unwrap(h.open(path, "open"));
});
test("corruption and permanent path failures have stable non-retryable errors", (t) => {
  const h = harness(t),
    path = join(h.directory, "private-garbage.sqlite");
  writeFileSync(path, "secret invalid database", { mode: 0o600 });
  assert.deepEqual(h.open(path, "open"), {
    ok: false,
    error: { code: "storage_corrupt", retry: "never" },
  });
  assert.deepEqual(h.open(join(h.directory, "missing.sqlite"), "open"), {
    ok: false,
    error: { code: "invalid_input", retry: "never" },
  });
  assert.deepEqual(h.open(path, "create"), {
    ok: false,
    error: { code: "invalid_input", retry: "never" },
  });
});
test("extended SQLite errors preserve primary reasons and never expose native values", async (t) => {
  const h = harness(t),
    store = unwrap(h.open()),
    db = h.raw(store),
    prepare = db.prepare.bind(db);
  const cases = [
    [{ errcode: 5 | (3 << 8) }, "unavailable", "same_command"],
    [{ errcode: 6 | (1 << 8) }, "unavailable", "same_command"],
    [{ errcode: 11 | (2 << 8) }, "storage_corrupt", "never"],
    [{ errcode: 26 }, "storage_corrupt", "never"],
    [{ errcode: 13 }, "limit_exceeded", "never"],
    [{ errcode: 8 | (3 << 8) }, "permission_denied", "never"],
    [{ code: "EACCES" }, "permission_denied", "never"],
    [{ errcode: 10 | (1 << 8) }, "unavailable", "never"],
    [{}, "unavailable", "never"],
  ];
  try {
    for (const [native, code, retry] of cases) {
      db.prepare = () => {
        throw Object.assign(new Error("private-path SELECT secret"), native);
      };
      assert.deepEqual(await store.session(fixtureSession().namespace), {
        ok: false,
        error: { code, retry },
      });
    }
  } finally {
    db.prepare = prepare;
  }
});

test("owned schema and persisted wire corruption are never reported as caller input", async (t) => {
  const h = harness(t),
    path = join(h.directory, "owned-corrupt.sqlite"),
    store = unwrap(h.open(path));
  const initial = fixtureSession();
  unwrap(await store.create(initial));
  const db = h.raw(store);
  db.prepare(
    "UPDATE sessions SET json=json_set(json,'$.schemaVersion',99)",
  ).run();
  assert.deepEqual(await store.session(initial.namespace), {
    ok: false,
    error: { code: "storage_corrupt", retry: "never" },
  });
  db.prepare("UPDATE schema_meta SET checksum='altered'").run();
  unwrap(await store.close(budget()));
  const bytes = readFileSync(path);
  assert.deepEqual(h.open(path, "open"), {
    ok: false,
    error: { code: "storage_corrupt", retry: "never" },
  });
  assert.deepEqual(readFileSync(path), bytes);
});
