import { spawnSync } from "node:child_process";
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  mkdtempSync,
  mkdirSync,
  writeFileSync,
  existsSync,
  readFileSync,
  rmSync,
  cpSync,
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

test("formal Make targets cannot inherit preview mode", () => {
  const root = mkdtempSync(join(tmpdir(), "agent-make-"));
  try {
    cpSync(new URL("../Makefile", import.meta.url), join(root, "Makefile"));
    writeFileSync(join(root, "node"), '#!/bin/sh\nprintf "%s" "$CI_PLAN"\n', {
      mode: 0o755,
    });
    for (const target of ["ci", "ci-full", "ci-plan"]) {
      const result = spawnSync("make", ["-s", target], {
        cwd: root,
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${root}:${process.env.PATH}`,
          CI_PLAN: "1",
        },
      });
      assert.equal(result.status, 0, result.stderr);
      assert.equal(result.stdout, target === "ci-plan" ? "1" : "0");
    }
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("dependency-using harness follows frozen install", () => {
  const names = steps.map(([name]) => name);
  assert.ok(
    names.indexOf("product harness tests") >
      names.indexOf("frozen dependencies"),
  );
});

test(
  "docs runner succeeds in a checkout with no node_modules",
  { skip: process.env.CI_FIXTURE_CHILD === "1" },
  () => {
    const root = mkdtempSync(join(tmpdir(), "agent-fresh-runner-"));
    try {
      cpSync(new URL("./", import.meta.url), join(root, "scripts"), {
        recursive: true,
      });
      cpSync(new URL("../Makefile", import.meta.url), join(root, "Makefile"));
      const [, command, args] = steps.find(
        ([name]) => name === "CI runner tests",
      );
      const result = spawnSync(command, args, {
        cwd: root,
        encoding: "utf8",
        env: { ...process.env, CI_FIXTURE_CHILD: "1" },
        maxBuffer: 8 * 1024 * 1024,
      });
      assert.equal(result.status, 0, result.stdout + result.stderr);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  },
);
