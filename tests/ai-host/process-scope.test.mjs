import { fixturePersistence } from "./harness.mjs";
import test from "node:test";
import assert from "node:assert/strict";
import { validScope } from "../../packages/ai-host/dist/launch-fence.js";
import {
  processWorkerAdapter,
  scopeAbsentWithin,
  WorkerPort,
} from "../../packages/ai-host/dist/process.js";
import { openSqliteStore } from "../../packages/ai-store-sqlite/dist/index.js";
import { createHost } from "../../packages/ai-host/dist/index.js";
import { fixtureSession } from "../../packages/ai-contract/dist/testing/index.js";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { preparedArtifact } from "../../scripts/preparation.mjs";
const unwrap = (result) => {
  assert.equal(result.ok, true, JSON.stringify(result));
  return result.value;
};
test("Windows persistent scope requires its OS login session", () => {
  const name = "Local\\rss-mdm-worker-00000000-0000-0000-0000-000000000000";
  assert.equal(validScope({ kind: "jobObject", name }), false);
  assert.equal(validScope({ kind: "jobObject", name, session: 1 }), true);
  assert.equal(validScope({ kind: "jobObject", name, session: -1 }), false);
});
test("a hanging scope probe cannot monopolize the Host event loop or close budget", async () => {
  const launcher = preparedArtifact("hostFixture");
  const before = performance.now();
  let tick = false;
  setTimeout(() => {
    tick = true;
  }, 5);
  assert.equal(
    await scopeAbsentWithin(
      { launcher, manifestDigest: "a".repeat(64) },
      { kind: "processGroup", root: 42 },
      { timeoutMs: 30, signal: new AbortController().signal },
    ),
    false,
  );
  assert.equal(tick, true);
  assert.ok(performance.now() - before < 1000);
});
test("startup failure before ownership registration retains an uncertain launch fence", async (t) => {
  const directory = await mkdtemp(join(tmpdir(), "rss-provisional-scope-"));
  const store = unwrap(
    await openSqliteStore({
      path: join(directory, "host.sqlite"),
      mode: "create",
    }),
  );
  t.after(async () => {
    await store.close({
      timeoutMs: 1000,
      signal: new AbortController().signal,
    });
    await rm(directory, { recursive: true, force: true });
  });
  const namespace = fixtureSession().namespace;
  const worker = new WorkerPort(
    { launcher: process.execPath, manifestDigest: "a".repeat(64) },
    store,
    namespace,
    new URL("./provider.mjs", import.meta.url).href,
  );
  const started = await worker.start(
    {
      namespace,
      provider: "fake",
      config: { id: "c", revision: "1" },
      workingDirectory: directory,
      permissions: "tools_disabled",
    },
    { timeoutMs: 1000, signal: new AbortController().signal },
  );
  assert.equal(started.ok, false);
  const launches = unwrap(await store.launches());
  assert.equal(launches.length, 1);
  assert.equal(
    launches[0].phase,
    process.platform === "win32" ? "reserved" : "registered",
  );
  const host = unwrap(
    await createHost({
      credentialPersistence: fixturePersistence(store),
      workers: processWorkerAdapter({
        launcher: process.execPath,
        manifestDigest: "a".repeat(64),
      }),
      delivery: null,
      store,
      launchFences: store,
      resolve: async () => {
        throw new Error("reserved launch must block restore");
      },
    }),
  );
  assert.equal(unwrap(await store.launches()).length, 1);
  unwrap(
    await host.close({
      timeoutMs: 1000,
      signal: new AbortController().signal,
    }),
  );
});

test("production Host keeps an uncertain namespace fenced while another caller stays responsive", async (t) => {
  const root = await mkdtemp(join(tmpdir(), "rss-host-probe-isolation-"));
  const store = unwrap(
    await openSqliteStore({ path: join(root, "host.sqlite"), mode: "create" }),
  );
  const session = fixtureSession();
  unwrap(await store.create(session));
  unwrap(
    await store.reserveLaunch({
      namespace: session.namespace,
      launchId: "uncertain",
      artifact: "fixture-provider",
      runtimeDigest: "a".repeat(64),
      phase: "reserved",
    }),
  );
  unwrap(
    await store.registerLaunch(session.namespace, "uncertain", {
      kind: "processGroup",
      root: 42,
    }),
  );
  let probe, started;
  const probing = new Promise((resolve) => {
    started = resolve;
  });
  let first = true;
  const host = unwrap(
    await createHost({
      store,
      launchFences: store,
      credentialPersistence: fixturePersistence(store),
      delivery: null,
      workers: {
        create() {
          throw Error("unknown fence must not launch a worker");
        },
        async absent() {
          if (first) {
            first = false;
            return false;
          }
          started();
          return new Promise((resolve) => {
            probe = resolve;
          });
        },
      },
      resolve: async () => {
        throw Error("unknown fence must not resolve a provider");
      },
    }),
  );
  t.after(async () => {
    probe?.(false);
    await host.close({ timeoutMs: 1000, signal: new AbortController().signal });
    await store.close({
      timeoutMs: 1000,
      signal: new AbortController().signal,
    });
    await rm(root, { recursive: true, force: true });
  });
  // resume enters the actual production admission/recovery path for the fenced namespace.
  const pending = host.resume(session.namespace, session.namespace.sessionId, {
    timeoutMs: 1000,
    signal: new AbortController().signal,
  });
  await probing;
  const other = { ...session.namespace, principalId: "other-caller" };
  const response = await Promise.race([
    host.connections(other, {
      timeoutMs: 1000,
      signal: new AbortController().signal,
    }),
    new Promise((_, reject) =>
      setTimeout(() => reject(Error("unrelated caller blocked by probe")), 500),
    ),
  ]);
  assert.equal(response.ok, true, JSON.stringify(response));
  probe(false);
  assert.equal((await pending).ok, false);
  assert.equal(unwrap(await store.launches()).length, 1);
});
