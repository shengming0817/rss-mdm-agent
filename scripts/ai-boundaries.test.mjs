import assert from "node:assert/strict";
import { test } from "node:test";
import {
  mkdtempSync,
  cpSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
  symlinkSync,
  rmSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

test("AI boundary gate covers app dependencies, builtins and computed imports", (t) => {
  const root = fileURLToPath(new URL("../", import.meta.url));
  const fixture = mkdtempSync(join(tmpdir(), "rss-ai-boundaries-"));
  t.after(() => rmSync(fixture, { recursive: true, force: true }));
  for (const target of [
    "packages/ai-contract",
    "packages/ai-host",
    "packages/ai-access",
    "packages/ai-client",
    "packages/ai-ui-bridge",
    "apps/ai-host",
  ]) {
    mkdirSync(join(fixture, target), { recursive: true });
    for (const entry of ["src", "package.json"])
      cpSync(join(root, target, entry), join(fixture, target, entry), {
        recursive: true,
      });
  }
  mkdirSync(join(fixture, "scripts"));
  cpSync(
    join(root, "scripts/check-ai-boundaries.mjs"),
    join(fixture, "scripts/check-ai-boundaries.mjs"),
  );
  symlinkSync(join(root, "node_modules"), join(fixture, "node_modules"));
  const run = () =>
    spawnSync(
      process.execPath,
      [join(fixture, "scripts/check-ai-boundaries.mjs")],
      { encoding: "utf8" },
    );
  const source = join(fixture, "apps/ai-host/src/index.ts"),
    original = readFileSync(source, "utf8");
  for (const injected of [
    'import cp from "node:child_process";',
    'import sdk from "@anthropic-ai/claude-agent-sdk";',
    "void import(input.artifact);",
    "const worker = require(process.env.MODULE);",
    'import x from "../../../packages/ai-host/src/process.js";',
  ]) {
    assert.equal(run().status, 0, "unmodified tree passes");
    writeFileSync(source, original + "\n" + injected);
    const failed = run();
    assert.notEqual(failed.status, 0, injected);
    assert.match(failed.stderr, /apps\/ai-host/);
    writeFileSync(source, original);
  }
  const manifest = join(fixture, "apps/ai-host/package.json");
  const packageJson = JSON.parse(readFileSync(manifest, "utf8"));
  for (const mutate of [
    (m) => {
      m.dependencies["@rss-mdm-agent/ai-host"] = "*";
    },
    (m) => {
      m.dependencies["@anthropic-ai/claude-agent-sdk"] = "0.3.277";
    },
    (m) => {
      m.optionalDependencies = { "node-pty": "1.0.0" };
    },
  ]) {
    assert.equal(run().status, 0);
    const changed = structuredClone(packageJson);
    mutate(changed);
    writeFileSync(manifest, JSON.stringify(changed));
    assert.notEqual(run().status, 0);
    writeFileSync(manifest, JSON.stringify(packageJson));
  }
});
