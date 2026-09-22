import { execFileSync, spawn, spawnSync } from "node:child_process";
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
import { runCI } from "./ci.mjs";
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

test("docs execute runner/docs only and never invoke skipped test commands", async () => {
  const plan = planSteps(steps, impact());
  assert.deepEqual(
    plan.filter((s) => s.selected).map((s) => s.name),
    ["CI runner tests", "docs and diff"],
  );
  const called = [];
  const results = await executeSteps(plan, "/unused", (command, args) => {
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

test("full preserves every gate, collection continues after failure and spawn errors", async () => {
  const plan = planSteps(steps, { ...impact(), full: true });
  assert.ok(plan.every((s) => s.selected));
  let count = 0;
  const results = await executeSteps(plan, "/unused", () => {
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

function lifecycleFixture(
  version = "11.4.0",
  nodeVersion = process.versions.node,
) {
  const base = mkdtempSync(join(tmpdir(), "ci-lifecycle-"));
  const root = join(base, "repo"),
    bin = join(base, "bin");
  mkdirSync(root);
  mkdirSync(bin);
  writeFileSync(
    join(root, "package.json"),
    JSON.stringify({
      engines: { node: nodeVersion },
      packageManager: "pnpm@11.4.0",
    }),
  );
  writeFileSync(join(root, ".gitignore"), ".local-ci-runs/\n");
  writeFileSync(join(bin, "pnpm"), `#!/bin/sh\nprintf '%s\\n' '${version}'\n`, {
    mode: 0o755,
  });
  const git = (...args) =>
    execFileSync("/usr/bin/git", args, {
      cwd: root,
      encoding: "utf8",
      stdio: "pipe",
    }).trim();
  git("init", "-q");
  git("add", ".");
  git(
    "-c",
    "user.name=Test",
    "-c",
    "user.email=test@example.invalid",
    "commit",
    "-qm",
    "fixture",
  );
  git("update-ref", "refs/remotes/origin/develop", "HEAD");
  mkdirSync(join(root, ".local-ci-runs"));
  writeFileSync(
    join(root, ".local-ci-runs/latest.json"),
    '{"status":"passed","sha":"old"}',
  );
  return {
    root,
    env: { ...process.env, PATH: `${bin}:${process.env.PATH}` },
    head: git("rev-parse", "HEAD"),
    read: () =>
      JSON.parse(
        readFileSync(join(root, ".local-ci-runs/latest.json"), "utf8"),
      ),
    close: () => rmSync(base, { recursive: true, force: true }),
  };
}

test("wrong Node or pnpm produces a failed receipt before executing any gate", async () => {
  for (const [pnpm, node] of [
    ["99.0.0", process.versions.node],
    ["11.4.0", "0.0.0"],
  ]) {
    const f = lifecycleFixture(pnpm, node);
    let executed = false;
    try {
      const code = await runCI(f.root, {
        env: f.env,
        steps: [["sentinel", "node", []]],
        execute: () => {
          executed = true;
          return { status: 0 };
        },
      });
      assert.notEqual(code, 0);
      assert.equal(executed, false);
      const receipt = f.read();
      assert.equal(receipt.status, "failed");
      assert.equal(receipt.sha, f.head);
      assert.match(receipt.error, /version/);
    } finally {
      f.close();
    }
  }
});

test("runner writes running progress and a failed terminal receipt on top-level exceptions", async () => {
  const f = lifecycleFixture();
  try {
    const code = await runCI(f.root, {
      env: f.env,
      steps: [["sentinel", "node", []]],
      execute: () => {
        assert.equal(f.read().status, "running");
        throw Error("injected spawn error");
      },
    });
    assert.notEqual(code, 0);
    assert.equal(f.read().status, "failed");
    assert.ok(
      f.read().results.some((r) => r.error?.includes("injected spawn error")),
    );
  } finally {
    f.close();
  }
});

test(
  "SIGTERM during a real gate preserves completed results and records cancellation",
  { timeout: 15000 },
  async () => {
    const f = lifecycleFixture();
    let child;
    try {
      const moduleURL = new URL("./ci.mjs", import.meta.url).href;
      const sequence = [
        ["first", process.execPath, ["-e", "process.exit(0)"]],
        [
          "wait",
          process.execPath,
          ["-e", "console.log('GATE_READY');setInterval(()=>{},1000)"],
        ],
        ["never", process.execPath, ["-e", "process.exit(0)"]],
      ];
      child = spawn(
        process.execPath,
        [
          "--input-type=module",
          "-e",
          `import {runCI} from ${JSON.stringify(moduleURL)};process.exitCode=await runCI(${JSON.stringify(f.root)},{steps:${JSON.stringify(sequence)}});`,
        ],
        { env: f.env, stdio: ["ignore", "pipe", "pipe"] },
      );
      let stdout = "",
        stderr = "",
        sent = false;
      child.stderr.on("data", (data) => (stderr += data));
      child.stdout.on("data", (data) => {
        stdout += data;
        if (!sent && /(^|\n)GATE_READY\r?\n/.test(stdout)) {
          sent = true;
          child.kill("SIGTERM");
        }
      });
      const code = await new Promise((resolve, reject) => {
        child.once("error", reject);
        child.once("close", resolve);
      });
      assert.notEqual(code, 0, stderr);
      const receipt = f.read();
      assert.equal(receipt.status, "cancelled");
      assert.equal(receipt.signal, "SIGTERM");
      assert.equal(
        receipt.results.find((r) => r.name === "first").outcome,
        "passed",
      );
      assert.equal(
        receipt.results.find((r) => r.name === "never").outcome,
        "skipped",
      );
    } finally {
      child?.kill("SIGKILL");
      f.close();
    }
  },
);

test("runner publishes passed and malformed prerequisites publish failed", async () => {
  const f = lifecycleFixture();
  try {
    assert.equal(
      await runCI(f.root, {
        env: f.env,
        steps: [["success", process.execPath, ["-e", "process.exit(0)"]]],
      }),
      0,
    );
    assert.equal(f.read().status, "passed");
    writeFileSync(join(f.root, "package.json"), "malformed json");
    assert.notEqual(await runCI(f.root, { env: f.env, steps: [] }), 0);
    assert.equal(f.read().status, "failed");
    assert.ok(f.read().error);
  } finally {
    f.close();
  }
});

test("real spawn failure is recorded and later gates still run", async () => {
  const results = await executeSteps(
    [
      {
        name: "missing",
        command: "rss-ci-no-such-command",
        args: [],
        selected: true,
      },
      {
        name: "later",
        command: process.execPath,
        args: ["-e", "process.exit(0)"],
        selected: true,
      },
    ],
    process.cwd(),
  );
  assert.equal(results[0].outcome, "failed");
  assert.match(results[0].error, /ENOENT/);
  assert.equal(results[1].outcome, "passed");
});
