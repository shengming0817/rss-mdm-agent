import assert from "node:assert/strict";
import { test } from "node:test";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createHost } from "../../packages/ai-host/dist/index.js";
import { WorkerPort, groupEmpty } from "../../packages/ai-host/dist/process.js";
import { openSqliteStore } from "../../packages/ai-store-sqlite/dist/index.js";
import {
  fixtureSession,
  seedSurface,
  acceptance,
  fixtureCommand,
} from "../../packages/ai-contract/dist/testing/index.js";
const budget = (timeoutMs = 5000) => ({
  timeoutMs,
  signal: new AbortController().signal,
});
const unwrap = (result) => {
  assert.equal(result.ok, true, JSON.stringify(result));
  return result.value;
};
async function until(check) {
  const deadline = Date.now() + 5000;
  while (Date.now() < deadline) {
    if (await check()) return;
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
  assert.fail("recovery deadline");
}
async function crash(t, mode) {
  const directory = await mkdtemp(join(tmpdir(), "rss-host-crash-"));
  const child = spawn(
    process.execPath,
    [new URL("./crash-driver.mjs", import.meta.url).pathname, directory, mode],
    { stdio: ["ignore", "pipe", "pipe"] },
  );
  t.after(async () => {
    if (child.exitCode === null && child.signalCode === null)
      child.kill("SIGKILL");
  });
  let stderr = "";
  child.stderr.on("data", (chunk) => (stderr += chunk));
  const ready = await new Promise((resolve, reject) => {
    let buffer = "";
    const timer = setTimeout(
      () => reject(new Error("child deadline " + stderr)),
      10000,
    );
    child.stdout.on("data", (chunk) => {
      buffer += chunk;
      if (buffer.includes("\n")) {
        clearTimeout(timer);
        resolve(JSON.parse(buffer.split("\n")[0]));
      }
    });
    child.once("exit", () => {
      clearTimeout(timer);
      reject(new Error("child exit " + stderr));
    });
  });
  child.kill("SIGKILL");
  await once(child, "exit");
  const pid = ready.pid ?? ready.launches[0].rootPid;
  await until(() => groupEmpty(pid));
  return { directory, ready, pid };
}
test("Host SIGKILL closes worker group; restart reconciles the original attempt without resending", async (t) => {
  const { directory, ready } = await crash(t, "unknown");
  const store = unwrap(
    openSqliteStore({ path: join(directory, "host.sqlite"), mode: "open" }),
  );
  const host = unwrap(
    await createHost({
      store,
      resolve: async (caller, options, namespace) => ({
        configuration: {
          namespace,
          provider: options.provider,
          config: options.config,
          accountRef: options.accountRef,
          workingDirectory: directory,
          permissions: "tools_disabled",
        },
        artifact: new URL("./provider.mjs", import.meta.url).href,
      }),
    }),
  );
  t.after(async () => {
    unwrap(await host.close(budget()));
    await rm(directory, { recursive: true, force: true });
  });
  await until(async () => {
    const trace = (await readFile(join(directory, "trace.ndjson"), "utf8"))
      .trim()
      .split("\n")
      .map(JSON.parse);
    return trace.some((row) => row.type === "reconcile");
  });
  const record = unwrap(
    await store.command(ready.session.namespace, "uncertain"),
  );
  assert.equal(record.dispatch.attemptId, ready.record.dispatch.attemptId);
  assert.equal(
    record.dispatch.originGeneration,
    ready.record.dispatch.originGeneration,
  );
  assert.notEqual(
    record.dispatch.observerGeneration,
    ready.record.dispatch.observerGeneration,
  );
  assert.equal(record.state, "reconciliation_required");
  assert.equal(
    (await readFile(join(directory, "trace.ndjson"), "utf8"))
      .split("\n")
      .filter((line) => line.includes('"type":"dispatch"')).length,
    1,
  );
});
test("crash after durable registration cannot import the SDK before activation", async (t) => {
  const { directory } = await crash(t, "registered");
  t.after(() => rm(directory, { recursive: true, force: true }));
  await assert.rejects(
    readFile(join(directory, "trace.ndjson")),
    (error) => error.code === "ENOENT",
  );
  const store = unwrap(
    openSqliteStore({ path: join(directory, "host.sqlite"), mode: "open" }),
  );
  const host = unwrap(
    await createHost({
      store,
      resolve: async () => {
        throw new Error("no session to resume");
      },
    }),
  );
  assert.deepEqual(unwrap(await store.launches()), []);
  unwrap(await host.close(budget()));
});
test("registration precedes provider import, and rejected registration leaves no runnable SDK", async (t) => {
  const directory = await mkdtemp(join(tmpdir(), "rss-host-fence-"));
  t.after(() => rm(directory, { recursive: true, force: true }));
  const store = unwrap(
    openSqliteStore({ path: join(directory, "host.sqlite"), mode: "create" }),
  );
  t.after(() => store.close(budget()));
  const namespace = fixtureSession().namespace;
  store.registerLaunch = async () => {
    await assert.rejects(
      readFile(join(directory, "trace.ndjson")),
      (error) => error.code === "ENOENT",
    );
    return { ok: false, error: { code: "unavailable", retry: "never" } };
  };
  const worker = new WorkerPort(
    store,
    namespace,
    new URL("./provider.mjs", import.meta.url).href,
  );
  const started = await worker.start(
    {
      namespace,
      provider: "fake",
      config: { id: "config", revision: "1" },
      accountRef: "account",
      workingDirectory: directory,
      permissions: "tools_disabled",
    },
    budget(),
  );
  assert.equal(started.ok, false);
  assert.deepEqual(unwrap(await store.launches()), []);
  await assert.rejects(
    readFile(join(directory, "trace.ndjson")),
    (error) => error.code === "ENOENT",
  );
});
test("blocked SDK activation is killed with real process-exit evidence", async (t) => {
  const directory = await mkdtemp(join(tmpdir(), "rss-host-block-"));
  t.after(() => rm(directory, { recursive: true, force: true }));
  const store = unwrap(
    openSqliteStore({ path: join(directory, "host.sqlite"), mode: "create" }),
  );
  t.after(() => store.close(budget()));
  const namespace = fixtureSession().namespace,
    worker = new WorkerPort(
      store,
      namespace,
      new URL("./provider.mjs", import.meta.url).href,
    );
  assert.equal(
    (
      await worker.start(
        {
          namespace,
          provider: "fake",
          config: { id: "config", revision: "activation_block" },
          accountRef: "account",
          workingDirectory: directory,
          permissions: "tools_disabled",
        },
        budget(300),
      )
    ).ok,
    false,
  );
  const pid = JSON.parse(
    (await readFile(join(directory, "trace.ndjson"), "utf8")).trim(),
  ).pid;
  assert.equal(groupEmpty(pid), true);
  assert.deepEqual(unwrap(await store.launches()), []);
});
test("Host crash also terminates a retained native grandchild despite a false adapter stopped claim", async (t) => {
  const { directory, pid } = await crash(t, "unknown_grandchild");
  t.after(() => rm(directory, { recursive: true, force: true }));
  const trace = (await readFile(join(directory, "trace.ndjson"), "utf8"))
      .trim()
      .split("\n")
      .map(JSON.parse),
    grandchild = trace.find((row) => row.type === "grandchild").childPid;
  assert.equal(groupEmpty(pid), true);
  assert.throws(
    () => process.kill(grandchild, 0),
    (error) => error.code === "ESRCH",
  );
});
test("unresolved registered process group freezes recovery without signaling or starting another worker", async (t) => {
  const directory = await mkdtemp(join(tmpdir(), "rss-host-orphan-")),
    child = spawn(process.execPath, ["-e", "setInterval(()=>{},1000)"], {
      detached: true,
      stdio: "ignore",
    });
  let host;
  t.after(async () => {
    await host?.close(budget());
    if (child.exitCode === null && child.signalCode === null) {
      child.kill("SIGKILL");
      await once(child, "exit");
    }
    await rm(directory, { recursive: true, force: true });
  });
  const path = join(directory, "host.sqlite"),
    store = unwrap(openSqliteStore({ path, mode: "create" })),
    session = fixtureSession();
  unwrap(await store.create(session));
  unwrap(await store.accept(acceptance(session)));
  unwrap(
    await store.reserveLaunch({
      namespace: session.namespace,
      launchId: "old-launch",
      artifact: "file:///trusted/provider.mjs",
      phase: "reserved",
    }),
  );
  unwrap(
    await store.registerLaunch(
      session.namespace,
      "old-launch",
      child.pid,
      child.pid,
    ),
  );
  unwrap(await store.close(budget()));
  const reopened = unwrap(openSqliteStore({ path, mode: "open" }));
  let resolves = 0;
  host = unwrap(
    await createHost({
      store: reopened,
      resolve: async () => {
        resolves++;
        throw new Error("must not start");
      },
    }),
  );
  assert.equal(resolves, 0);
  assert.equal(child.exitCode, null);
  assert.equal(groupEmpty(child.pid), false);
  const snapshot = unwrap(
    await reopened.snapshotPage(session.namespace, { limit: 256 }),
  );
  assert.equal(snapshot.session.status, "recovery_required");
  assert.equal(snapshot.commands[0].state, "accepted");
  assert.deepEqual(snapshot.session.binding, session.binding);
  assert.equal(
    unwrap(await reopened.listSessions(session.namespace, { limit: 256 }))
      .items[0].status,
    "recovery_required",
  );
  assert.equal(unwrap(await reopened.launches()).length, 1);
});
for (const hasSession of [true, false])
  test(`launch recovery includes ${hasSession ? "idle session" : "pre-session"} fences and reclaims exited groups on admission`, async (t) => {
    const directory = await mkdtemp(join(tmpdir(), "rss-host-idle-fence-"));
    const child = spawn(process.execPath, ["-e", "setInterval(()=>{},1000)"], {
      detached: true,
      stdio: "ignore",
    });
    const path = join(directory, "host.sqlite"),
      store = unwrap(openSqliteStore({ path, mode: "create" })),
      session = fixtureSession();
    let host;
    t.after(async () => {
      await host?.close(budget());
      if (child.exitCode === null && child.signalCode === null) {
        child.kill("SIGKILL");
        await once(child, "exit");
      }
      await rm(directory, { recursive: true, force: true });
    });
    if (hasSession) unwrap(await store.create(session));
    unwrap(
      await store.reserveLaunch({
        namespace: session.namespace,
        launchId: "idle-launch",
        artifact: "file:///trusted/provider.mjs",
        phase: "reserved",
      }),
    );
    unwrap(
      await store.registerLaunch(
        session.namespace,
        "idle-launch",
        child.pid,
        child.pid,
      ),
    );
    unwrap(await store.close(budget()));
    const reopened = unwrap(openSqliteStore({ path, mode: "open" }));
    host = unwrap(
      await createHost({
        store: reopened,
        resolve: async () => {
          throw new Error("fixture admission refused");
        },
      }),
    );
    assert.equal(groupEmpty(child.pid), false);
    if (hasSession)
      assert.equal(
        unwrap(await reopened.session(session.namespace)).status,
        "recovery_required",
      );
    child.kill("SIGKILL");
    await once(child, "exit");
    await until(() => groupEmpty(child.pid));
    await host.createSession(
      session.namespace,
      {
        provider: "fake",
        config: { id: "c", revision: "1" },
        accountRef: "a",
        profile: "conversation",
      },
      budget(),
    );
    assert.deepEqual(unwrap(await reopened.launches()), []);
  });
test("worker close cannot finish before an outstanding launch reservation settles", async (t) => {
  const directory = await mkdtemp(join(tmpdir(), "rss-worker-start-close-"));
  const store = unwrap(
    openSqliteStore({ path: join(directory, "host.sqlite"), mode: "create" }),
  );
  t.after(async () => {
    await store.close(budget());
    await rm(directory, { recursive: true, force: true });
  });
  const reserve = store.reserveLaunch.bind(store);
  let release,
    entered = false;
  const held = new Promise((resolve) => {
    release = resolve;
  });
  store.reserveLaunch = async (input) => {
    entered = true;
    await held;
    return reserve(input);
  };
  const namespace = fixtureSession().namespace,
    worker = new WorkerPort(
      store,
      namespace,
      new URL("./provider.mjs", import.meta.url).href,
    );
  const started = worker.start(
    {
      namespace,
      provider: "fake",
      config: { id: "c", revision: "1" },
      accountRef: "a",
      workingDirectory: directory,
      permissions: "tools_disabled",
    },
    budget(),
  );
  await until(() => entered);
  const closing = worker.close(budget(100));
  await new Promise((resolve) => setTimeout(resolve, 150));
  release();
  const [start, stop] = await Promise.all([started, closing]);
  assert.equal(start.ok, false);
  assert.equal(stop.value.processStopped, false);
  assert.equal(unwrap(await worker.close(budget())).processStopped, true);
  assert.deepEqual(unwrap(await store.launches()), []);
  await assert.rejects(
    readFile(join(directory, "trace.ndjson")),
    (error) => error.code === "ENOENT",
  );
});
test("recovery unavailability atomically preserves queues and invalidates stale callbacks and controls", async (t) => {
  const directory = await mkdtemp(join(tmpdir(), "rss-host-unavailable-")),
    store = unwrap(
      openSqliteStore({ path: join(directory, "host.sqlite"), mode: "create" }),
    );
  t.after(async () => {
    unwrap(await store.close(budget()));
    await rm(directory, { recursive: true, force: true });
  });
  const seeded = await seedSurface(store),
    namespace = seeded.session.namespace;
  let head = unwrap(await store.session(namespace));
  unwrap(
    await store.accept(
      acceptance(head, { ...fixtureCommand(), commandId: "queued" }),
    ),
  );
  head = unwrap(await store.session(namespace));
  unwrap(
    await store.accept(
      acceptance(head, {
        ...fixtureCommand(),
        commandId: "control",
        input: {
          type: "cancel",
          generation: head.binding.generation,
          targetCommandId: "command-1",
          nativeRunId: head.binding.nativeRunId,
        },
      }),
    ),
  );
  head = unwrap(await store.session(namespace));
  const recovered = unwrap(
    await store.recoverUnavailable({
      namespace,
      expectedRevision: head.revision,
      expectedGeneration: head.binding.generation,
      eventId: "recover-unavailable",
    }),
  );
  const snapshot = unwrap(await store.snapshotPage(namespace, { limit: 256 }));
  assert.deepEqual(recovered.binding, head.binding);
  assert.equal(recovered.status, "recovery_required");
  assert.equal(
    snapshot.commands.find((row) => row.command.commandId === "queued").state,
    "accepted",
  );
  assert.equal(
    snapshot.commands.find((row) => row.command.commandId === "control").state,
    "invalidated",
  );
  assert.equal(
    snapshot.commands.find((row) => row.command.commandId === "command-1")
      .state,
    "reconciliation_required",
  );
  assert.equal(snapshot.interactions[0].status, "unavailable");
  assert.equal(snapshot.surfaces[0].status, "invalidated");
});
