import assert from "node:assert/strict";
import test from "node:test";
import { getEventListeners } from "node:events";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { withinBudget } from "../../packages/ai-contract/dist/budget.js";

test("completed operation budgets do not accumulate on a long-lived caller", async () => {
  if (!global.gc) {
    const env = { ...process.env };
    delete env.NODE_TEST_CONTEXT;
    const child = spawnSync(
      process.execPath,
      [
        "--expose-gc",
        "--test",
        "--test-name-pattern=^completed operation budgets",
        fileURLToPath(import.meta.url),
      ],
      { env, encoding: "utf8", timeout: 15000 },
    );
    assert.equal(child.status, 0, child.stdout + child.stderr);
    assert.match(child.stdout, /completed operation budgets/);
    return;
  }
  const caller = new AbortController();
  const timers = () =>
    process.getActiveResourcesInfo().filter((name) => name === "Timeout")
      .length;
  const initialTimers = timers();
  for (let i = 0; i < 2; i++)
    await withinBudget(
      () => ({ timeoutMs: 30000, signal: caller.signal }),
      () => i,
    );
  await new Promise(setImmediate);
  global.gc();
  assert.equal(getEventListeners(caller.signal, "abort").length, 0);
  // Node 24.14.1 regression sentinel; listener counts alone miss composite dependencies.
  const dependency = Object.getOwnPropertySymbols(caller.signal).find(
    (key) => key.description === "kDependantSignals",
  );
  assert.equal(caller.signal[dependency]?.size ?? 0, 0);
  assert.ok(
    timers() <= initialTimers,
    "settled operations leave no watchdog timers",
  );
});

test("caller and owner cancellation independently stop a scoped operation", async () => {
  for (const sourceName of ["caller", "owner"]) {
    const caller = new AbortController(),
      owner = new AbortController();
    const source = sourceName === "caller" ? caller : owner;
    let observed;
    await assert.rejects(
      withinBudget(
        () => ({ timeoutMs: 30000, signal: caller.signal }),
        (budget) => {
          observed = budget.signal;
          source.abort();
          return new Promise(() => {});
        },
        owner.signal,
      ),
      /budget exhausted/,
    );
    assert.equal(observed.aborted, true);
    assert.equal(getEventListeners(caller.signal, "abort").length, 0);
    assert.equal(getEventListeners(owner.signal, "abort").length, 0);
    await assert.rejects(
      withinBudget(
        () => ({ timeoutMs: 30000, signal: caller.signal }),
        () => assert.fail("already cancelled scopes must not start work"),
        owner.signal,
      ),
      /budget exhausted/,
    );
  }
});

test("operation budget releases ownership on success, throw, abort and timeout", async () => {
  const caller = new AbortController();
  const factory = () => ({ timeoutMs: 20, signal: caller.signal });
  for (const work of [
    () => 7,
    () => {
      throw new Error("sync");
    },
    () => Promise.reject(new Error("async")),
    () => new Promise(() => {}),
  ]) {
    let child;
    try {
      await withinBudget(factory, (budget) => {
        child = budget.signal;
        return work();
      });
    } catch {}
    assert.equal(child.aborted, true);
    assert.equal(getEventListeners(caller.signal, "abort").length, 0);
    assert.equal(caller.signal.aborted, false);
  }
  let child;
  const waiting = withinBudget(
    () => ({ timeoutMs: 30000, signal: caller.signal }),
    (budget) => {
      child = budget.signal;
      caller.abort();
      return new Promise(() => {});
    },
  );
  await assert.rejects(waiting, /budget exhausted/);
  assert.equal(child.aborted, true);
  assert.equal(getEventListeners(caller.signal, "abort").length, 0);
  await assert.rejects(
    withinBudget(factory, () =>
      assert.fail("aborted caller must not start work"),
    ),
    /budget exhausted/,
  );
});
