import { activeStage } from "../../packages/ai-contract/dist/index.js";
import { readSnapshot } from "../../packages/ai-contract/dist/testing/index.js";
import assert from "node:assert/strict";
import test from "node:test";
import { spawn } from "node:child_process";
import { DatabaseSync } from "node:sqlite";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { createInterface } from "node:readline";
import {
  fixtureSession,
  acceptance,
  unwrap,
  restoredSession,
  emptyCommit,
  commandCommit,
  verifiedReconciliation,
} from "../../packages/ai-contract/dist/testing/index.js";
import { harness, budget } from "./support.mjs";
const worker = fileURLToPath(new URL("./worker.mjs", import.meta.url));
function start(config) {
  const child = spawn(process.execPath, [worker, JSON.stringify(config)], {
    stdio: ["ignore", "pipe", "pipe"],
  });
  const exited = new Promise((resolve) =>
    child.once("exit", (code, signal) => resolve({ code, signal })),
  );
  const line = new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      child.kill("SIGKILL");
      reject(new Error("worker stage timeout"));
    }, 10000);
    const lines = createInterface({ input: child.stdout });
    lines.once("line", (text) => {
      clearTimeout(timer);
      lines.close();
      try {
        resolve(JSON.parse(text));
      } catch {
        reject(new Error("invalid worker result"));
      }
    });
    child.once("exit", () => {
      clearTimeout(timer);
      reject(new Error("worker exited before stage"));
    });
    child.once("error", reject);
  });
  return {
    child,
    line,
    exited,
    async kill() {
      if (child.exitCode === null && child.signalCode === null)
        child.kill("SIGKILL");
      return exited;
    },
  };
}
test("an idle Host excludes a second process; SIGKILL releases ownership without reopening old-generation writes", async (t) => {
  const h = harness(t),
    path = join(h.directory, "owned.sqlite");
  const owner = start({ path, mode: "create", scenario: "hold" });
  try {
    assert.deepEqual(await owner.line, { stage: "idle-owner" });
    const contender = start({ path, mode: "open", scenario: "probe" });
    const result = await contender.line;
    assert.deepEqual(
      { ok: result.ok, error: result.error },
      { ok: false, error: { code: "unavailable", retry: "same_command" } },
    );
    assert.ok(
      result.elapsedMs < 1000,
      `50ms busy wait exceeded bounded margin: ${result.elapsedMs}`,
    );
    assert.deepEqual(await contender.exited, { code: 0, signal: null });
  } finally {
    await owner.kill();
  }
  const next = unwrap(h.open(path, "open")),
    initial = fixtureSession();
  const head = unwrap(await next.session(initial.namespace));
  assert.equal(
    (await next.accept(acceptance(head))).error.code,
    "stale_binding",
  );
  const restored = await restoredSession(head, "owner-successor");
  const rebound = unwrap(
    await next.rebind({
      namespace: head.namespace,
      expectedRevision: head.revision,
      expectedGeneration: activeStage(head).binding.generation,
      restored,
      eventId: "successor",
    }),
  );
  assert.equal((await next.commit(emptyCommit(head))).ok, false);
  unwrap(await next.accept(acceptance(rebound)));
  unwrap(await next.close(budget()));
  const third = start({ path, mode: "open", scenario: "probe" });
  assert.equal((await third.line).ok, true);
  assert.deepEqual(await third.exited, { code: 0, signal: null });
});
for (const scenario of [
  "accept-before-commit",
  "accept-after-commit",
  "before-publish",
  "intent-before-commit",
  "intent-after-commit",
  "unknown",
]) {
  test(`SIGKILL at ${scenario} preserves exactly the committed prefix`, async (t) => {
    const h = harness(t),
      path = join(h.directory, "crash.sqlite"),
      process = start({ path, mode: "create", scenario });
    try {
      await process.line;
    } finally {
      assert.equal((await process.kill()).signal, "SIGKILL");
    }
    const store = unwrap(h.open(path, "open")),
      initial = fixtureSession();
    const snapshot = unwrap(await readSnapshot(store, initial.namespace));
    const persisted = scenario !== "accept-before-commit";
    assert.equal(snapshot.commands.length, persisted ? 1 : 0);
    assert.equal(snapshot.events.length, snapshot.cursor);
    assert.deepEqual(
      snapshot.events.map((e) => e.sequence),
      snapshot.events.map((_, i) => i + 1),
    );
    if (persisted) {
      const receipt = unwrap(await store.accept(acceptance(initial)));
      assert.deepEqual(receipt, snapshot.commands[0].receipt);
      assert.equal(
        unwrap(await readSnapshot(store, initial.namespace)).cursor,
        snapshot.cursor,
        "lost receipt never appends acceptance twice",
      );
    }
    if (scenario === "intent-before-commit") {
      assert.equal(snapshot.commands[0].state, "accepted");
      assert.equal(snapshot.commands[0].dispatch, undefined);
      assert.equal(snapshot.cursor, 1);
      assert.equal(snapshot.session.revision, 1);
      const restored = await restoredSession(
        snapshot.session,
        "intent-successor",
      );
      const current = unwrap(
        await store.rebind({
          namespace: initial.namespace,
          expectedRevision: snapshot.session.revision,
          expectedGeneration: activeStage(initial).binding.generation,
          restored,
          eventId: "intent-rollback-rebind",
        }),
      );
      const attempt = {
        attemptId: "fresh-after-rollback",
        originGeneration: activeStage(current).binding.generation,
        observerGeneration: activeStage(current).binding.generation,
        nativeSessionId: activeStage(current).binding.nativeSessionId,
        certainty: "intent",
      };
      const dispatching = {
        ...snapshot.commands[0],
        state: "dispatching",
        dispatch: attempt,
      };
      unwrap(
        await store.commit(
          commandCommit(current, dispatching, [
            { type: "dispatch", attempt },
            { type: "status", state: "dispatching" },
          ]),
        ),
      );
      assert.deepEqual(
        unwrap(await store.command(initial.namespace, "command-1")),
        dispatching,
      );
    }
    if (scenario === "intent-after-commit" || scenario === "unknown") {
      const dispatch = snapshot.commands[0].dispatch;
      assert.equal(
        dispatch.certainty,
        scenario === "unknown" ? "unknown" : "intent",
      );
      if (scenario === "unknown")
        assert.equal(dispatch.correlationId, "lookup-command-1");
      const restored = await restoredSession(
        snapshot.session,
        "crash-successor",
      );
      const head = unwrap(
        await store.rebind({
          namespace: initial.namespace,
          expectedRevision: snapshot.session.revision,
          expectedGeneration: activeStage(initial).binding.generation,
          restored,
          eventId: "crash-rebind",
        }),
      );
      const record = unwrap(
        await store.command(initial.namespace, "command-1"),
      );
      assert.equal(record.state, "reconciliation_required");
      assert.deepEqual(record.dispatch, {
        ...dispatch,
        observerGeneration: "crash-successor",
      });
      unwrap(await store.close(budget()));
      const reopened = unwrap(h.open(path, "open"));
      const restoredAgain = await restoredSession(head, "crash-third");
      const third = unwrap(
        await reopened.rebind({
          namespace: head.namespace,
          expectedRevision: head.revision,
          expectedGeneration: activeStage(head).binding.generation,
          restored: restoredAgain,
          eventId: "crash-third-rebind",
        }),
      );
      const latest = unwrap(
        await reopened.command(head.namespace, "command-1"),
      );
      assert.deepEqual(latest.dispatch, {
        ...dispatch,
        observerGeneration: "crash-third",
      });
      const accepted = {
        schemaVersion: 5,
        kind: "commandRecord",
        command: record.command,
        receipt: record.receipt,
        state: "accepted",
      };
      assert.equal(
        (await reopened.commit(commandCommit(third, accepted, []))).ok,
        false,
        "crash uncertainty never authorizes resubmission",
      );
      const proofCommit = commandCommit(third, accepted, [
        {
          type: "reconciled",
          attempt: latest.dispatch,
          resolution: "not_submitted",
        },
      ]);
      proofCommit.events[0].attemptId = dispatch.attemptId;
      unwrap(
        await reopened.commit({
          ...proofCommit,
          nowMs: 1,
          providerFacts: [
            await verifiedReconciliation(third, latest, "not_submitted"),
          ],
        }),
      );
      const retryHead = unwrap(await reopened.session(third.namespace));
      const intent = {
        attemptId: dispatch.attemptId,
        originGeneration: "crash-third",
        observerGeneration: "crash-third",
        nativeSessionId: dispatch.nativeSessionId,
        certainty: "intent",
      };
      const fresh = (attempt) =>
        commandCommit(
          retryHead,
          { ...accepted, state: "dispatching", dispatch: attempt },
          [
            { type: "dispatch", attempt },
            { type: "status", state: "dispatching" },
          ],
        );
      assert.equal(
        (await reopened.commit({ ...fresh(intent), nowMs: 1 })).ok,
        false,
      );
      const newAttempt = { ...intent, attemptId: "after-two-restarts" };
      unwrap(await reopened.commit({ ...fresh(newAttempt), nowMs: 1 }));
      const history = unwrap(await reopened.events(third.namespace, 0, 1024));
      assert.deepEqual(
        history.findLast((e) => e.body.type === "reconciled").body.attempt,
        latest.dispatch,
      );
    }
  });
}

test("SIGKILL during initial migration leaves no partially installed schema", async (t) => {
  const h = harness(t),
    path = join(h.directory, "migration-kill.sqlite");
  const child = start({
    path,
    mode: "create",
    scenario: "migration-before-commit",
  });
  try {
    assert.deepEqual(await child.line, { stage: "migration-before-commit" });
  } finally {
    assert.equal((await child.kill()).signal, "SIGKILL");
  }
  const raw = new DatabaseSync(path);
  try {
    assert.equal(raw.prepare("PRAGMA user_version").get().user_version, 0);
    assert.equal(
      raw.prepare("SELECT count(*) AS n FROM sqlite_schema").get().n,
      0,
    );
  } finally {
    raw.close();
  }
  assert.equal(h.open(path, "open").error.code, "unsupported_version");
});
