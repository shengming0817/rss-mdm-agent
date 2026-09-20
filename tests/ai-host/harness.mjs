import { unwrap } from "../../packages/ai-contract/dist/testing/index.js";
/** An already admitted scripted context for process/recovery tests, never a product API. */
export async function openFixture(host, store, caller, options, budget) {
  const connectionId = [
    options.config.id,
    options.accountRef,
    options.config.revision,
  ].join(":");
  const found = await store.connection(caller, connectionId);
  if (!found.ok)
    unwrap(
      await store.saveConnection(
        caller,
        {
          schemaVersion: 5,
          kind: "connection",
          connectionId,
          name: connectionId,
          provider: "codex",
          configRevision: 1,
          credentialRevision: 1,
          accountRef: options.accountRef,
          profile: options.profile,
          status: "ready",
          source: {
            type: "custom_api",
            apiUrl: "https://example.invalid/v1",
            model: options.config.revision,
          },
          credentialRef: "fixture-key",
        },
        null,
      ),
    );
  const created = await host.createSession(caller, { connectionId }, budget);
  // The harness deliberately establishes the precondition without adding a user command.
  return created.ok
    ? host.result(() =>
        host.admit(created.value.namespace, budget, (b) =>
          host.ensureContext(created.value, b),
        ),
      )
    : created;
}
export async function fixtureArtifact(store, namespace, options) {
  const row = await store.connection(
    namespace,
    options.config.id,
    Number(options.config.revision),
  );
  const artifact = new URL("./provider.mjs", import.meta.url);
  if (row.ok) artifact.searchParams.set("scenario", row.value.source.model);
  return artifact.href;
}
