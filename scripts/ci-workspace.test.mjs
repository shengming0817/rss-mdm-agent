import { test } from "node:test";
import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import { loadGraph, selectImpact } from "./ci-impact.mjs";
const npm = (name) => `@rss-mdm-agent/${name}`;
test("real workspace resolves all metadata and source/test bridges", () => {
  const graph = loadGraph(fileURLToPath(new URL("../", import.meta.url)));
  assert.ok(graph.rust.has("rss-mdm-desktop"));
  for (const [path, owner] of [
    ["apps/ai-host/src/execution-tools.json", "execution-mcp"],
    ["apps/desktop/src/assistant/execution-types.ts", "execution-app"],
    ["tests/assistant/execution-fixtures.json", "execution-app"],
  ])
    assert.ok(
      graph.roots.some(([p, n]) => p === path && n === owner),
      path,
    );

  assert.ok(graph.reverse.get(npm("ai-contract")).has("ai-session-contract"));
  assert.ok(graph.reverse.get("execution-app").has(npm("ai-host-app")));
  for (const adapter of ["claude", "codex", "deepseek"]) {
    assert.ok(
      graph.roots.some(
        ([path, owner]) =>
          path === "tests/ai-provider-conformance" &&
          owner === npm(`ai-adapter-${adapter}`),
      ),
    );
  }
});

// A launcher edit must rebuild the application artifact that embeds it.
test("dirty native launcher selects real Host packaging", async (t) => {
  const { mkdtempSync, mkdirSync, writeFileSync, rmSync } = await import(
    "node:fs"
  );
  const { tmpdir } = await import("node:os");
  const { join } = await import("node:path");
  const { execFileSync } = await import("node:child_process");
  const { planSteps } = await import("./ci-plan.mjs");
  const { steps } = await import("./ci-steps.mjs");
  const graph = loadGraph(fileURLToPath(new URL("../", import.meta.url)));
  const root = mkdtempSync(join(tmpdir(), "agent-native-impact-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const git = (...args) =>
    execFileSync("/usr/bin/git", args, {
      cwd: root,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    }).trim();
  git("init", "-b", "topic");
  git(
    "-c",
    "user.name=Test",
    "-c",
    "user.email=test@example.invalid",
    "commit",
    "--allow-empty",
    "-m",
    "base",
  );
  const baseRef = git("rev-parse", "HEAD");
  mkdirSync(join(root, "crates/native-process/src"), { recursive: true });
  writeFileSync(
    join(root, "crates/native-process/src/lib.rs"),
    "// changed launcher",
  );
  const impact = selectImpact(root, { baseRef, graph: () => graph });
  assert.equal(impact.full, false);
  const plan = planSteps(steps, impact);
  assert.equal(
    plan.find((step) => step.name === "AI Host local runtime").selected,
    true,
  );
  assert.equal(
    plan.find((step) => step.name === "AI provider acceptance").selected,
    true,
  );
});
