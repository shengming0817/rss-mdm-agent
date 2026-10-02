import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, linkSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { openSqliteStore } from "../../packages/ai-store-sqlite/dist/index.js";
test("a database hard-link alias is rejected without changing the original", async (t) => {
  const root = mkdtempSync(join(tmpdir(), "ai-store-alias-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const path = join(root, "ai.sqlite");
  const created = openSqliteStore({ path, mode: "create" });
  assert.equal(created.ok, true);
  await created.value.close({
    timeoutMs: 1000,
    signal: new AbortController().signal,
  });
  const before = readFileSync(path),
    alias = join(root, "alias.sqlite");
  linkSync(path, alias);
  assert.equal(openSqliteStore({ path: alias, mode: "open" }).ok, false);
  assert.deepEqual(readFileSync(path), before);
});
