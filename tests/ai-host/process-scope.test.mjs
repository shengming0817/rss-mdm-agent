import test from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { validScope } from "../../packages/ai-host/dist/launch-fence.js";
import {
  scopeAbsentWithin,
  WorkerPort,
} from "../../packages/ai-host/dist/process.js";
import { openSqliteStore } from "../../packages/ai-store-sqlite/dist/index.js";
import { createHost } from "../../packages/ai-host/dist/index.js";
import { fixtureSession } from "../../packages/ai-contract/dist/testing/index.js";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
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
  execFileSync(
    "cargo",
    ["build", "--locked", "-p", "native-process", "--example", "host-fixture"],
    { stdio: "ignore" },
  );
  const launcher = fileURLToPath(
    new URL(
      "../../target/debug/examples/host-fixture" +
        (process.platform === "win32" ? ".exe" : ""),
      import.meta.url,
    ),
  );
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
    openSqliteStore({ path: join(directory, "host.sqlite"), mode: "create" }),
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
      workerRuntime: {
        launcher: process.execPath,
        manifestDigest: "a".repeat(64),
      },
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
