import { activeStage } from "../../packages/ai-contract/dist/index.js";
import { openFixture, fixtureArtifact } from "./harness.mjs";
import { createHost } from "../../packages/ai-host/dist/index.js";
import { openSqliteStore } from "../../packages/ai-store-sqlite/dist/index.js";
import { join } from "node:path";
const [directory, mode] = process.argv.slice(2);
const unwrap = (result) => {
  if (!result.ok) throw new Error(result.error.code);
  return result.value;
};
const store = unwrap(
  openSqliteStore({ path: join(directory, "host.sqlite"), mode: "create" }),
);
if (mode === "registered") {
  const register = store.registerLaunch.bind(store);
  store.registerLaunch = async (...args) => {
    const result = await register(...args);
    process.stdout.write(
      JSON.stringify({ phase: "registered", pid: args[2] }) + "\n",
    );
    await new Promise(() => {});
    return result;
  };
}
const caller = {
  tenantId: "tenant",
  principalId: "person",
  authorityId: "authority",
};
const options = {
  provider: "fake",
  config: { id: "config", revision: mode === "registered" ? "1" : mode },
  accountRef: "account",
  profile: "conversation",
};
const host = unwrap(
  await createHost({
    delivery: null,
    store,
    launchFences: store,
    resolve: async (caller, options, namespace) => ({
      configuration: {
        namespace,
        provider: options.provider,
        config: options.config,
        accountRef: options.accountRef,
        workingDirectory: directory,
        permissions: "tools_disabled",
      },
      artifact: await fixtureArtifact(store, namespace, options),
    }),
  }),
);
const budget = { timeoutMs: 5000, signal: new AbortController().signal };
const session = unwrap(await openFixture(host, store, caller, options, budget));
unwrap(
  await host.submit(
    caller,
    {
      schemaVersion: 5,
      kind: "command",
      sessionId: session.namespace.sessionId,
      commandId: "uncertain",
      expiresAtMs: Date.now() + 60000,
      input: { type: "prompt", policy: "queue_next", text: "hold" },
    },
    budget,
  ),
);
for (;;) {
  const record = unwrap(await store.command(session.namespace, "uncertain"));
  if (record.dispatch?.certainty === "unknown") {
    process.stdout.write(
      JSON.stringify({
        session,
        record,
        launches: unwrap(await store.launches()),
      }) + "\n",
    );
    break;
  }
  await new Promise((resolve) => setTimeout(resolve, 10));
}
setInterval(() => {}, 1000);
