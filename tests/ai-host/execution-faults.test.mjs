import assert from "node:assert/strict";
import { fork, spawn } from "node:child_process";
import { once } from "node:events";
import { mkdtemp, readFile, realpath, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { connectExecution } from "../../apps/ai-host/dist/execution.js";
import { Deliveries } from "../../packages/ai-host/dist/delivery.js";
import { openSqliteStore } from "../../packages/ai-store-sqlite/dist/index.js";
import {
  fixtureCommand,
  fixtureSession,
  restoredSession,
  unwrap,
} from "../../packages/ai-contract/dist/testing/index.js";

import { executionServer } from "./rust-execution.mjs";

const budget = (timeoutMs = 10_000) => ({
  timeoutMs,
  signal: new AbortController().signal,
});
const binding = {
  caller: {
    tenantId: "s1-test",
    principalId: "fixture-actor",
    authorityId: "desktop-fixture",
  },
  session: {
    provider: "codex",
    accountRef: "test-account",
    config: { id: "local", revision: "r1" },
    profile: "controlled_tools",
  },
};
const mailbox = () => {
  let tail = Promise.resolve();
  return (_, action) => {
    const task = tail.then(action);
    tail = task.catch(() => {});
    return task;
  };
};
const owner = (store, router, now = Date.now) =>
  new Deliveries(store, router, mailbox(), async () => {}, now);
const reply = (result) => JSON.parse(unwrap(result).text);

async function crashAfterRustAcceptance(
  t,
  path,
  session,
  command,
  proposal,
  router,
) {
  const prepared = unwrap(router.prepare(session.namespace, proposal));
  const child = fork(
    new URL("./execution-crash-driver.mjs", import.meta.url),
    [
      path,
      JSON.stringify(session),
      JSON.stringify(command),
      JSON.stringify(proposal),
      JSON.stringify(prepared),
    ],
    { stdio: ["ignore", "pipe", "pipe", "ipc"] },
  );
  t.after(() => {
    if (child.exitCode === null && child.signalCode === null)
      child.kill("SIGKILL");
  });
  let stderr = "";
  child.stderr.on("data", (chunk) => (stderr += chunk));
  const accepted = new Promise((resolve, reject) => {
    const timer = setTimeout(
      () => reject(new Error(`driver timeout: ${stderr}`)),
      20_000,
    );
    child.on("message", async (message) => {
      if (message?.type !== "send") return;
      try {
        await router.send(message.request, budget());
        const reconciled = await router.reconcile(message.request, budget());
        assert.equal(reconciled.ok, true, JSON.stringify(reconciled));
        assert.equal(reconciled.value.state, "committed");
        clearTimeout(timer);
        resolve(message.request);
      } catch (error) {
        clearTimeout(timer);
        reject(error);
      }
    });
    child.once("exit", (code, signal) => {
      clearTimeout(timer);
      reject(
        new Error(
          `driver exited before Rust acceptance: ${code}/${signal}: ${stderr}`,
        ),
      );
    });
  });
  const request = await accepted;
  child.kill("SIGKILL");
  await once(child, "exit");
  return request;
}

test("lost submit receipt recovers the same Rust attempt and keeps process exit separate from cancellation", async (t) => {
  const executable = executionServer();
  const root = await realpath(
    await mkdtemp(join(tmpdir(), "rss-execution-fault-")),
  );
  const rustDb = join(root, "execution.sqlite");
  const aiDb = join(root, "ai.sqlite");
  const audit = join(root, "execution-audit.json");
  const rust = spawn(executable, [rustDb, audit, "ai-unknown"], {
    cwd: new URL("../..", import.meta.url),
    stdio: ["pipe", "pipe", "pipe"],
  });
  let rustError = "";
  rust.stderr.on("data", (chunk) => (rustError += chunk));
  let connection;
  let store;
  t.after(async () => {
    await connection?.close().catch(() => {});
    rust.stdin.end();
    if (rust.exitCode === null && rust.signalCode === null)
      rust.kill("SIGKILL");
    await store?.close(budget()).catch(() => {});
    await rm(root, { recursive: true, force: true });
  });
  try {
    connection = await connectExecution(rust.stdout, rust.stdin, binding);
  } catch (error) {
    throw new Error(`${error.message}: ${rustError}`);
  }

  const original = fixtureSession();
  const session = {
    ...original,
    namespace: { ...binding.caller, sessionId: "conversation-a" },
    binding: {
      ...original.binding,
      provider: "codex",
      accountRef: "test-account",
      config: { id: "local", revision: "r1" },
      generation: "generation-a",
    },
    capabilities: {
      ...original.capabilities,
      continuation: "across_processes",
    },
  };
  const plan = JSON.parse(await readFile(audit, "utf8"));

  const submitCommand = {
    ...fixtureCommand("submit-command"),
    sessionId: session.namespace.sessionId,
    expiresAtMs: Date.now() + 60_000,
  };
  const submit = {
    name: "execution_submit",
    arguments: {
      operationRequestId: "ai-unknown",
      plan,
    },
  };
  const request = await crashAfterRustAcceptance(
    t,
    aiDb,
    session,
    submitCommand,
    submit,
    connection.router,
  );

  store = unwrap(openSqliteStore({ path: aiDb, mode: "open" }));
  const crashed = unwrap(await store.session(session.namespace));
  const successor = unwrap(
    await store.rebind({
      namespace: session.namespace,
      expectedRevision: crashed.revision,
      expectedGeneration: crashed.binding.generation,
      restored: await restoredSession(crashed, "generation-b"),
      eventId: "execution-fault-takeover",
    }),
  );
  assert.equal(successor.binding.generation, "generation-b");
  let modelCommand = unwrap(
    await store.command(session.namespace, submitCommand.commandId),
  );
  assert.equal(modelCommand.state, "reconciliation_required");
  assert.equal(modelCommand.outcome, undefined);
  assert.equal(modelCommand.dispatch.originGeneration, "generation-a");
  assert.equal(modelCommand.dispatch.observerGeneration, "generation-b");
  let saved = unwrap(
    await store.delivery(session.namespace, request.body.operationId),
  );
  assert.equal(saved.delivery.status, "reconciliation_required");
  assert.equal(saved.delivery.attempts, 1);
  const restarted = owner(store, connection.router, () => Date.now() + 2_000);
  await restarted.recover(budget());
  saved = unwrap(
    await store.delivery(session.namespace, request.body.operationId),
  );
  assert.equal(saved.delivery.status, "delivered");
  assert.equal(
    saved.delivery.attempts,
    1,
    "unknown recovery must not resend submit",
  );
  modelCommand = unwrap(
    await store.command(session.namespace, submitCommand.commandId),
  );
  assert.equal(modelCommand.state, "reconciliation_required");
  assert.equal(modelCommand.outcome, undefined);
  assert.equal(modelCommand.dispatch.originGeneration, "generation-a");
  assert.equal(modelCommand.dispatch.observerGeneration, "generation-b");

  const duplicate = reply(
    await restarted.propose(
      session.namespace,
      successor.binding.generation,
      submitCommand.commandId,
      submit,
      budget(),
    ),
  );
  assert.equal(duplicate.status, "ok");
  assert.equal(duplicate.result.phase, "outcomeUnknown");
  assert.equal(duplicate.result.cancelRequested, false);
  saved = unwrap(
    await store.delivery(session.namespace, request.body.operationId),
  );
  assert.equal(
    saved.delivery.attempts,
    1,
    "duplicate receipt must stay idempotent",
  );

  const cancelProposal = {
    name: "execution_cancel",
    arguments: { operationRequestId: "ai-unknown" },
  };
  const cancelPrepared = unwrap(
    connection.router.prepare(session.namespace, cancelProposal),
  );
  const cancelReceipt = unwrap(
    await connection.router.send(
      {
        ...request,
        eventId: "cancel-event",
        commandId: "cancel-command",
        body: {
          type: "delivery_requested",
          operationId: cancelPrepared.operationId,
          target: cancelPrepared.target,
          proposal: cancelProposal,
        },
      },
      budget(),
    ),
  );
  const cancelled = JSON.parse(cancelReceipt.reply.text);
  assert.equal(cancelled.status, "ok");
  assert.equal(cancelled.result.operation.cancelRequested, true);
  assert.equal(cancelled.result.operation.phase, "outcomeUnknown");

  await connection.close();
  rust.stdin.end();
  const [code] = await once(rust, "exit");
  assert.equal(code, 0, rustError);
  const evidence = JSON.parse(await readFile(audit, "utf8"));
  assert.equal(evidence.attempts, 1);
  assert.equal(evidence.cancelRequested, true);
  assert.equal(evidence.phase, "outcomeUnknown");
});
