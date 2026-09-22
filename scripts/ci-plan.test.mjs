import { test } from "node:test";
import assert from "node:assert/strict";
import {
  mkdtempSync,
  mkdirSync,
  writeFileSync,
  existsSync,
  readFileSync,
  rmSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { steps } from "./ci-steps.mjs";
import {
  planSteps,
  executeSteps,
  prepareEvidence,
  publishPlan,
} from "./ci-plan.mjs";
const impact = (rustPackages = [], nodePackages = []) => ({
  full: false,
  rustPackages,
  nodePackages,
  packages: [...rustPackages, ...nodePackages],
});

test("docs execute runner/docs only and never invoke skipped test commands", () => {
  const plan = planSteps(steps, impact());
  assert.deepEqual(
    plan.filter((s) => s.selected).map((s) => s.name),
    ["CI runner tests", "docs and diff"],
  );
  const called = [];
  const results = executeSteps(plan, "/unused", (command, args) => {
    called.push([command, ...args]);
    return { status: 0 };
  });
  assert.equal(called.length, 2);
  assert.equal(results.find((r) => r.name === "rust test").outcome, "skipped");
  assert.notEqual(results.find((r) => r.name === "rust test").status, 0);
});

test("affected Cargo packages replace workspace and JS build includes producer closure", () => {
  const plan = planSteps(steps, impact(["script-plan"], ["@rss-mdm-agent/ui"]));
  assert.deepEqual(plan.find((s) => s.name === "rust test").args, [
    "test",
    "-p",
    "script-plan",
    "--locked",
  ]);
  assert.deepEqual(plan.find((s) => s.name === "workspace build inputs").args, [
    "--filter",
    "@rss-mdm-agent/ui...",
    "build",
  ]);
  assert.equal(
    plan.find((s) => s.name === "Claude SDK adapter").selected,
    false,
  );
  assert.equal(plan.find((s) => s.name === "packed consumer").selected, true);
});

test("full preserves every gate, collection continues after failure and spawn errors", () => {
  const plan = planSteps(steps, { ...impact(), full: true });
  assert.ok(plan.every((s) => s.selected));
  let count = 0;
  const results = executeSteps(plan, "/unused", () => {
    count++;
    if (count === 1) throw Error("spawn failure");
    return { status: count === 2 ? 7 : 0 };
  });
  assert.equal(count, steps.length);
  assert.equal(results[0].outcome, "failed");
  assert.equal(results[1].status, 7);
  assert.equal(results.at(-1).outcome, "passed");
});

test("plan preserves formal evidence; execution retires gate receipts but retains manual acceptance", () => {
  const root = mkdtempSync(join(tmpdir(), "agent-ci-plan-"));
  const out = join(root, ".local-ci-runs");
  mkdirSync(out);
  try {
    for (const name of [
      "latest.json",
      "selection.json",
      "codex-consumer.json",
      "rust-consumers.json",
      "desktop-native.json",
    ])
      writeFileSync(join(out, name), "old success");
    const plan = planSteps(steps, impact());
    publishPlan(root, { impact: impact(), steps: plan }, true);
    assert.equal(readFileSync(join(out, "latest.json"), "utf8"), "old success");
    assert.equal(
      readFileSync(join(out, "selection.json"), "utf8"),
      "old success",
    );
    prepareEvidence(root);
    for (const name of [
      "latest.json",
      "selection.json",
      "codex-consumer.json",
      "rust-consumers.json",
    ])
      assert.equal(existsSync(join(out, name)), false, name);
    assert.equal(existsSync(join(out, "desktop-native.json")), true);
    assert.equal(existsSync(join(out, "plan.json")), true);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
