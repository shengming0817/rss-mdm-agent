import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { openSqliteStore } from "../../packages/ai-store-sqlite/dist/index.js";
export const budget = () => ({
  timeoutMs: 1000,
  signal: new AbortController().signal,
});
export function harness(t) {
  const directory = mkdtempSync(join(tmpdir(), "rss-ai-store-")),
    stores = [],
    connections = new WeakMap();
  t.after(async () => {
    for (const s of stores) await s.close(budget());
    rmSync(directory, { recursive: true, force: true });
  });
  let next = 0;
  return {
    directory,
    raw: (store) => connections.get(store),
    open(
      path = join(directory, `store-${next++}.sqlite`),
      mode = "create",
      options = {},
    ) {
      const prepare = DatabaseSync.prototype.prepare;
      let connection;
      DatabaseSync.prototype.prepare = function (...args) {
        connection = this;
        return prepare.apply(this, args);
      };
      try {
        const result = openSqliteStore({ path, mode, ...options });
        if (result.ok) {
          stores.push(result.value);
          connections.set(result.value, connection);
        }
        return result;
      } finally {
        DatabaseSync.prototype.prepare = prepare;
      }
    },
  };
}
