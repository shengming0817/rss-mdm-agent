import { activeStage } from "../../packages/ai-contract/dist/index.js";
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
    prepare: () =>
      ok({ operationId: "submission", target: "rust", permission: "ask" }),
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
  const owner = (authorize = async () => ok("allowed")) =>
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
      authorize,
      () => true,
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
  name: "execution_execute",
  arguments: {
    operationRequestId: "request",
    plan: { requestId: "plan", digest: "0".repeat(64) },
  },
};
test("delivery survives an ended run and SQLite reopen; unknown never causes blind resend", async (t) => {
  const f = await setup(t);
  let owner = f.owner();
  assert.equal(
    (
      await owner.propose(
        f.session.namespace,
        activeStage(f.session).binding.generation,
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
        activeStage(f.session).binding.generation,
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
    activeStage(f.session).binding.generation,
    "command-1",
    {
      ...proposal,
      arguments: {
        ...proposal.arguments,
        plan: { requestId: "different", digest: "1".repeat(64) },
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
    ok({
      operationId: p.arguments.operationRequestId,
      target: "rust",
      permission: "ask",
    });
  for (const id of ["a-hung", "b-ready", "c-hung", "d-hung", "e-ready"])
    await owner.propose(
      f.session.namespace,
      activeStage(f.session).binding.generation,
      "command-1",
      {
        ...proposal,
        arguments: { ...proposal.arguments, operationRequestId: id },
      },
      budget(),
    );
  f.advance();
  const calls = new Map();
  f.router.reconcile = async (request) => {
    calls.set(
      request.body.operationId,
      (calls.get(request.body.operationId) ?? 0) + 1,
    );
    return request.body.operationId.endsWith("hung")
      ? new Promise(() => {})
      : ok({
          state: "committed",
          receipt: {
            receiptRef: `receipt-${request.body.operationId}`,
            reply: { disposition: "returned", text: "ready" },
          },
        });
  };
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
  // Wrap all recovery pages repeatedly, then retry the exact proposal. The original
  // router ignored abort and still owns the operation despite every wait expiring.
  for (let i = 0; i < 4; i++) await owner.recover(budget(20));
  await owner.propose(
    f.session.namespace,
    activeStage(f.session).binding.generation,
    "command-1",
    {
      ...proposal,
      arguments: { ...proposal.arguments, operationRequestId: "a-hung" },
    },
    budget(20),
  );
  for (const id of ["a-hung", "c-hung", "d-hung"])
    assert.equal(calls.get(id), 1, id);
});

for (const stage of ["send", "reconcile", "acknowledge"]) {
  test(`${stage} retains single flight until the actual call settles, not its waiter`, async (t) => {
    const f = await setup(t),
      owner = f.owner();
    const receipt = {
      receiptRef: "rust-receipt",
      reply: { disposition: "returned", text: "done" },
    };
    let calls = 0,
      release;
    f.router.send = async () => ok(receipt);
    f.router.reconcile = async () => ok({ state: "committed", receipt });
    f.router.acknowledge = async () => ok();
    f.router[stage] = () => {
      calls++;
      return new Promise((resolve) => {
        release = resolve;
      });
    };
    if (stage === "reconcile") {
      f.router.send = async () => ({
        ok: false,
        error: { code: "unavailable", retry: "reconcile_first" },
      });
    }
    const propose = (ms = 30) =>
      owner.propose(
        f.session.namespace,
        activeStage(f.session).binding.generation,
        "command-1",
        proposal,
        budget(ms),
      );
    assert.equal((await propose()).ok, false);
    if (stage === "reconcile") assert.equal((await propose()).ok, false);
    assert.equal(calls, 1);
    f.advance();
    await owner.recover(budget(20));
    await propose(20);
    assert.equal(calls, 1);
    const joined = propose(1000);
    release(
      stage === "send"
        ? ok(receipt)
        : stage === "reconcile"
          ? ok({ state: "committed", receipt })
          : ok(),
    );
    await joined;
    // A real settlement releases ownership even when the first budget has expired.
    f.router[stage] = async () =>
      stage === "send"
        ? ok(receipt)
        : stage === "reconcile"
          ? ok({ state: "committed", receipt })
          : ok();
    assert.equal((await propose(1000)).ok, true);
    assert.equal(
      unwrap(await f.store().delivery(f.session.namespace, "submission"))
        .delivery.status,
      "delivered",
    );
  });
}

test("new tool intent waits once outside the mailbox; reject never persists and approve rechecks the live command", async (t) => {
  const f = await setup(t);
  let answer,
    asked = 0;
  const owner = f.owner(() => {
    asked++;
    return new Promise((resolve) => {
      answer = resolve;
    });
  });
  const propose = (input = proposal) =>
    owner.propose(
      f.session.namespace,
      activeStage(f.session).binding.generation,
      "command-1",
      input,
      budget(),
    );
  const first = propose(),
    duplicate = propose();
  for (let i = 0; i < 50 && !answer; i++)
    await new Promise((r) => setTimeout(r, 1));
  assert.equal(asked, 1);
  assert.equal(
    unwrap(await f.store().delivery(f.session.namespace, "submission")),
    null,
  );
  assert.equal(
    (await propose({ ...proposal, arguments: { changed: true } })).error.code,
    "content_conflict",
  );
  answer(ok("rejected"));
  assert.equal(unwrap(await first).disposition, "rejected");
  assert.deepEqual(await duplicate, await first);
  assert.equal(f.count().send, 0);
  assert.equal(
    unwrap(await f.store().delivery(f.session.namespace, "submission")),
    null,
  );
  answer = undefined;
  const stale = propose();
  for (let i = 0; i < 50 && !answer; i++)
    await new Promise((r) => setTimeout(r, 1));
  const head = unwrap(await f.store().session(f.session.namespace));
  const command = unwrap(await f.store().command(head.namespace, "command-1"));
  unwrap(await f.store().commit(await terminalCommit(head, command)));
  answer(ok("allowed"));
  assert.equal((await stale).error.code, "stale_binding");
  assert.equal(f.count().send, 0);
});

test("permission timeout never writes intent; a durable exact retry and recovery do not ask again", async (t) => {
  const f = await setup(t);
  const hung = f.owner(() => new Promise(() => {}));
  assert.equal(
    (
      await hung.propose(
        f.session.namespace,
        activeStage(f.session).binding.generation,
        "command-1",
        proposal,
        budget(20),
      )
    ).ok,
    false,
  );
  assert.equal(
    unwrap(await f.store().delivery(f.session.namespace, "submission")),
    null,
  );
  let asked = 0;
  const owner = f.owner(async () => {
    asked++;
    return ok("allowed");
  });
  await owner.propose(
    f.session.namespace,
    activeStage(f.session).binding.generation,
    "command-1",
    proposal,
    budget(),
  );
  await owner.propose(
    f.session.namespace,
    activeStage(f.session).binding.generation,
    "command-1",
    proposal,
    budget(),
  );
  f.state("committed");
  f.advance();
  await owner.recover(budget());
  assert.equal(asked, 1);
  assert.equal(f.count().send, 1);
});
