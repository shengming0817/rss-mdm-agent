import { createHost } from "../../packages/ai-host/dist/index.js";
import { openSqliteStore } from "../../packages/ai-store-sqlite/dist/index.js";
import {
  readConfiguration,
  configurationFingerprint,
} from "../../apps/ai-host/dist/configuration.js";
import { unwrap } from "../ai-provider-conformance/support.mjs";

/** Test composition uses the production worker factory; only the Store commit boundary is instrumented. */
export async function openHost(path, mode, beforeCommit) {
  const local = await readConfiguration(path);
  const store = unwrap(openSqliteStore({ path: local.databasePath, mode }));
  if (beforeCommit) {
    const commit = store.commit.bind(store);
    store.commit = async (batch) => {
      await beforeCommit(store, batch);
      return commit(batch);
    };
  }
  const artifact = new URL(
    "../../apps/ai-host/dist/provider.js",
    import.meta.url,
  );
  artifact.searchParams.set("configuration", path);
  artifact.searchParams.set("fingerprint", configurationFingerprint(local));
  const diagnostics = [];
  const host = unwrap(
    await createHost({
      store,
      launchFences: store,
      delivery: null,
      onDiagnostic: (row) => diagnostics.push(row),
      resolve: async (caller, options, namespace) => ({
        configuration: {
          namespace,
          provider: options.provider,
          config: options.config,
          accountRef: options.accountRef,
          workingDirectory: local.workingDirectory,
          permissions: "tools_disabled",
        },
        artifact: artifact.href,
      }),
    }),
  );
  return { host, store, local, diagnostics };
}
