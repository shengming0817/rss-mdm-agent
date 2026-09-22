import { test } from "node:test";
import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import { loadGraph } from "./ci-impact.mjs";
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
