import { ConnectionSecrets } from "../../apps/ai-host/dist/secrets.js";
import { createHost } from "../../packages/ai-host/dist/index.js";
import { openSqliteStore } from "../../packages/ai-store-sqlite/dist/index.js";
import { readConfiguration } from "../../apps/ai-host/dist/configuration.js";
import { localResolver } from "../../apps/ai-host/dist/resolver.js";
import { unwrap } from "../ai-provider-conformance/support.mjs";

/** Test composition uses the production worker factory; only the Store commit boundary is instrumented. */
export async function openHost(path, mode, beforeCommit) {
  const local = await readConfiguration(path);
  const store = unwrap(
    openSqliteStore({ path: local.databasePath, mode: "open" }),
  );
  if (beforeCommit) {
    const commit = store.commit.bind(store);
    store.commit = async (batch) => {
      await beforeCommit(store, batch);
      return commit(batch);
    };
  }
  const diagnostics = [];
  const host = unwrap(
    await createHost({
      store,
      launchFences: store,
      delivery: null,
      onDiagnostic: (row) => diagnostics.push(row),
      resolve: localResolver(
        local,
        store,
        new ConnectionSecrets(store, async () => Buffer.alloc(32, 7)),
      ),
    }),
  );
  return {
    host,
    store,
    local,
    caller: {
      tenantId: "test-users",
      principalId: "fixture-actor",
      authorityId: "desktop-fixture",
    },
    diagnostics,
  };
}
