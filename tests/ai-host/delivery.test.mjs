import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { Deliveries } from "../../packages/ai-host/dist/delivery.js";
import { openSqliteStore } from "../../packages/ai-store-sqlite/dist/index.js";
import {
  fixtureSession,
  acceptance,
  dispatchCommand,
  terminalCommit,
  unwrap,
} from "../../packages/ai-contract/dist/testing/index.js";
const budget = (timeoutMs = 1000) => ({
  timeoutMs,
  signal: new AbortController().signal,
});
const ok = (value) => ({ ok: true, value });
async function setup(t) {
  const root = await mkdtemp(join(tmpdir(), "rss-delivery-"));
  let store;
  t.after(async () => {
    await store?.close(budget());
    await rm(root, { recursive: true, force: true });
  });
  const path = join(root, "ai.sqlite");
  store = unwrap(openSqliteStore({ path, mode: "create" }));
  let session = fixtureSession();
  unwrap(await store.create(session));
  unwrap(await store.accept(acceptance(session)));
  session = unwrap(await store.session(session.namespace));
  await dispatchCommand(store, session, "command-1", "submitted");
  session = unwrap(await store.session(session.namespace));
  let time = 2,
    send = 0,
    ack = 0,
    reconcile = 0,
    state = "unknown",
    failAck = false;
  const receipt = {
    receiptRef: "rust-receipt",
    reply: { disposition: "returned", text: "untrusted business reply" },
  };
  const router = {
    prepare: () => ok({ operationId: "submission", target: "rust" }),
    send: async () => {
      send++;
      return {
        ok: false,
        error: { code: "unavailable", retry: "reconcile_first" },
      };
    },
    reconcile: async () => {
      reconcile++;
      return ok(state === "committed" ? { state, receipt } : { state });
    },
    acknowledge: async () => {
      ack++;
      assert.equal(
        unwrap(await store.delivery(session.namespace, "submission")).delivery
          .status,
        "receipt_recorded",
      );
      return failAck
        ? {
            ok: false,
            error: { code: "unavailable", retry: "reconcile_first" },
          }
        : ok();
    },
  };
  let mailbox = Promise.resolve();
  const owner = () =>
    new Deliveries(
      store,
      router,
      (_, action) => {
        const task = mailbox.then(action);
        mailbox = task.catch(() => {});
        return task;
      },
      async () => {},
      () => time,
    );
  return {
    session,
    router,
    owner,
    store: () => store,
    count: () => ({ send, ack, reconcile }),
    state: (value) => {
      state = value;
    },
    failAck: (value) => {
      failAck = value;
    },
    advance: () => {
      time += 2000;
    },
    restart: async () => {
      unwrap(await store.close(budget()));
      store = unwrap(openSqliteStore({ path, mode: "open" }));
    },
  };
}
const proposal = {
  name: "execution_submit",
  arguments: {
    operationRequestId: "request",
    plan: { planId: "plan", digest: "0".repeat(64) },
  },
};
test("delivery survives an ended run and SQLite reopen; unknown never causes blind resend", async (t) => {
  const f = await setup(t);
  let owner = f.owner();
  assert.equal(
    (
      await owner.propose(
        f.session.namespace,
        f.session.binding.generation,
        "command-1",
        proposal,
        budget(),
      )
    ).ok,
    false,
  );
  assert.equal(f.count().send, 1);
  const head = unwrap(await f.store().session(f.session.namespace));
  const command = unwrap(await f.store().command(head.namespace, "command-1"));
  unwrap(await f.store().commit(await terminalCommit(head, command)));
  await f.restart();
  owner = f.owner();
  f.advance();
  await owner.recover(budget());
  assert.equal(f.count().send, 1);
  f.state("committed");
  f.failAck(true);
  await owner.recover(budget());
  assert.equal(
    unwrap(await f.store().delivery(head.namespace, "submission")).delivery
      .status,
    "receipt_recorded",
  );
  await f.restart();
  owner = f.owner();
  f.failAck(false);
  await owner.recover(budget());
  assert.equal(
    unwrap(await f.store().delivery(head.namespace, "submission")).delivery
      .status,
    "delivered",
  );
  assert.equal(f.count().send, 1);
  assert.equal(
    unwrap(await f.store().command(head.namespace, "command-1")).outcome,
    "completed",
  );
});
test("exact operation/content conflict is permanent and a hung receiver has a bounded wait", async (t) => {
  const f = await setup(t);
  const owner = f.owner();
  f.router.send = () => new Promise(() => {});
  const started = Date.now();
  assert.equal(
    (
      await owner.propose(
        f.session.namespace,
        f.session.binding.generation,
        "command-1",
        proposal,
        budget(30),
      )
    ).ok,
    false,
  );
  assert.ok(Date.now() - started < 500);
  const conflict = await owner.propose(
    f.session.namespace,
    f.session.binding.generation,
    "command-1",
    {
      ...proposal,
      arguments: {
        ...proposal.arguments,
        plan: { planId: "different", digest: "1".repeat(64) },
      },
    },
    budget(),
  );
  assert.equal(conflict.error.code, "content_conflict");
});

test("hung deliveries cannot starve ready operations or later pages", async (t) => {
  const f = await setup(t),
    owner = f.owner();
  f.router.prepare = (_, p) =>
    ok({ operationId: p.arguments.operationRequestId, target: "rust" });
  for (const id of ["a-hung", "b-ready", "c-hung", "d-hung", "e-ready"])
    await owner.propose(
      f.session.namespace,
      f.session.binding.generation,
      "command-1",
      {
        ...proposal,
        arguments: { ...proposal.arguments, operationRequestId: id },
      },
      budget(),
    );
  f.advance();
  f.router.reconcile = async (request) =>
    request.body.operationId.endsWith("hung")
      ? new Promise(() => {})
      : ok({
          state: "committed",
          receipt: {
            receiptRef: `receipt-${request.body.operationId}`,
            reply: { disposition: "returned", text: "ready" },
          },
        });
  f.router.acknowledge = async () => ok();
  const recovered = await owner.recover(budget(50));
  assert.equal(recovered.ok, false);
  assert.equal(
    unwrap(await f.store().delivery(f.session.namespace, "b-ready")).delivery
      .status,
    "delivered",
  );
  assert.equal(
    unwrap(await f.store().delivery(f.session.namespace, "a-hung")).delivery
      .status,
    "reconciliation_required",
  );
  assert.equal((await owner.recover(budget(50))).ok, true);
  assert.equal(
    unwrap(await f.store().delivery(f.session.namespace, "e-ready")).delivery
      .status,
    "delivered",
  );
  assert.equal(f.count().send, 5);
});
