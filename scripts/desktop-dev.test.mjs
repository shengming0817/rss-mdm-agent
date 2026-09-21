import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { mkdirSync, mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { test } from "node:test";
import {
  developmentFingerprint,
  ensureDevelopmentRuntime,
} from "./desktop-dev-runtime.mjs";

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), "rss-dev-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const write = (path, value) => {
    mkdirSync(join(root, path, ".."), { recursive: true });
    writeFileSync(join(root, path), value);
  };
  for (const path of [
    "package.json",
    "pnpm-lock.yaml",
    "pnpm-workspace.yaml",
    "scripts/bundle-ai-host.mjs",
    "scripts/ai-host-artifacts.mjs",
    "scripts/desktop-dev-runtime.mjs",
    "scripts/verify-ai-host-runtime.mjs",
  ])
    write(path, "{}");
  for (const path of [
    "apps/ai-host",
    "packages/ai-host",
    "packages/ai-contract",
    "packages/ai-store-sqlite",
    "packages/ai-access",
    "packages/ai-client",
    "packages/ai-adapters/claude",
    "packages/ai-adapters/codex",
    "packages/ai-adapters/deepseek",
  ])
    write(`${path}/src/index.ts`, "original");
  return { root, write };
}
test("fingerprint includes dirty Host, adapter, contract, lock, Node and added/deleted sources, excludes UI and outputs", (t) => {
  const { root, write } = fixture(t);
  const first = developmentFingerprint(root);
  write("apps/desktop/src/App.vue", "UI");
  write("apps/ai-host/dist/cli.js", "output");
  assert.equal(developmentFingerprint(root), first);
  for (const path of [
    "apps/ai-host/src/index.ts",
    "packages/ai-adapters/codex/src/index.ts",
    "packages/ai-contract/schema/runtime.schema.json",
    "pnpm-lock.yaml",
    "package.json",
  ]) {
    const before = developmentFingerprint(root);
    write(path, "changed");
    assert.notEqual(developmentFingerprint(root), before, path);
  }
  const before = developmentFingerprint(root);
  write("apps/ai-host/src/new.ts", "new");
  assert.notEqual(developmentFingerprint(root), before);
  rmSync(join(root, "apps/ai-host/src/new.ts"));
  assert.equal(developmentFingerprint(root), before);
});
test("missing and stale runtimes rebuild, unchanged runtime reuses and validates", (t) => {
  const { root, write } = fixture(t);
  let built = 0,
    verified = 0;
  const directory = join(root, "runtime");
  const options = {
    build() {
      built++;
      mkdirSync(directory, { recursive: true });
      write(
        "runtime/manifest.json",
        JSON.stringify({
          kind: "development",
          developmentFingerprint: developmentFingerprint(root),
        }),
      );
    },
    verify() {
      verified++;
    },
  };
  ensureDevelopmentRuntime(root, directory, options);
  assert.equal(built, 1);
  ensureDevelopmentRuntime(root, directory, options);
  assert.equal(built, 1);
  write("apps/ai-host/src/index.ts", "dirty");
  ensureDevelopmentRuntime(root, directory, options);
  assert.equal(built, 2);
  assert.equal(verified, 3);
});
test("build failures and invalid runtime abort preparation", (t) => {
  const { root } = fixture(t);
  const directory = join(root, "runtime");
  assert.throws(
    () =>
      ensureDevelopmentRuntime(root, directory, {
        build() {
          throw Error("missing dependency");
        },
      }),
    /missing dependency/,
  );
  assert.throws(
    () =>
      ensureDevelopmentRuntime(root, directory, {
        build() {},
        verify() {
          throw Error("invalid runtime");
        },
      }),
    /invalid runtime/,
  );
});

test("preparation rejects edits during a build and a corrupt reused runtime", (t) => {
  const { root, write } = fixture(t);
  const directory = join(root, "runtime");
  assert.throws(
    () =>
      ensureDevelopmentRuntime(root, directory, {
        build() {
          write("apps/ai-host/src/index.ts", "concurrent edit");
        },
        verify() {},
      }),
    /source changed/,
  );
  write(
    "runtime/manifest.json",
    JSON.stringify({
      kind: "development",
      developmentFingerprint: developmentFingerprint(root),
    }),
  );
  assert.throws(
    () =>
      ensureDevelopmentRuntime(root, directory, {
        build() {
          assert.fail("must not rebuild unchanged cache");
        },
        verify() {
          throw Error("runtime tree integrity mismatch");
        },
      }),
    /integrity mismatch/,
  );
});

test("invalid override fails before Tauri starts with actionable stage diagnostics", () => {
  const result = spawnSync(
    process.execPath,
    [fileURLToPath(new URL("./desktop-dev.mjs", import.meta.url))],
    {
      encoding: "utf8",
      env: { ...process.env, RSS_AI_HOST_RUNTIME: "" },
    },
  );
  assert.equal(result.status, 1);
  assert.match(result.stderr, /AI Host override validation failed/);
  assert.match(result.stderr, /non-empty runtime directory/);
});
