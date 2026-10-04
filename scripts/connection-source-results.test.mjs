import assert from "node:assert/strict";
import { test } from "node:test";
import { sourceSummary } from "./connection-source-results.mjs";
import { spawnSync } from "node:child_process";
import {
  cpSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { Preparation, contextKey } from "./preparation.mjs";
test("both official existing configurations require a completed probe; absent sources are not proof of support", () => {
  for (const provider of ["claude", "codex"]) {
    const row = {
      provider,
      source: "existing_config",
      result: "model_probe_completed",
    };
    assert.equal(sourceSummary([row]).status, "passed");
    assert.equal(
      sourceSummary([{ ...row, result: "unsupported_capability" }]).status,
      "failed",
    );
    assert.equal(
      sourceSummary([{ ...row, result: "authentication_required" }]).status,
      "failed",
    );
    assert.equal(
      sourceSummary([{ ...row, result: "source_absent" }]).status,
      "partial",
    );
  }
});

for (const mode of [
  "success",
  "drift",
  "publish-drift",
  "cancel",
  "retirement-cancel",
  "missing",
])
  test(`connection preflight publishes the final preparation outcome: ${mode}`, (t) => {
    const source = fileURLToPath(new URL("../", import.meta.url));
    const root = realpathSync(
      mkdtempSync(join(tmpdir(), "connection-preparation-")),
    );
    t.after(() => rmSync(root, { recursive: true, force: true }));
    mkdirSync(join(root, "scripts"));
    for (const name of [
      "check-connection-sources",
      "connection-source-results",
      "ai-host-artifacts",
      "preparation",
      "ci-plan",
      "ci-result",
      "cargo-target",
    ])
      cpSync(
        join(source, `scripts/${name}.mjs`),
        join(root, `scripts/${name}.mjs`),
      );
    cpSync(join(source, "package.json"), join(root, "package.json"));
    symlinkSync(join(source, "node_modules"), join(root, "node_modules"));
    mkdirSync(join(root, "crates/sample"), { recursive: true });
    writeFileSync(join(root, "crates/sample/input.rs"), "baseline");
    const host = join(root, "apps/ai-host/dist"),
      client = join(root, "packages/ai-client/dist");
    mkdirSync(host, { recursive: true });
    mkdirSync(client, { recursive: true });
    writeFileSync(
      join(host, "secrets.js"),
      "export const connectionPersistence = () => {};",
    );
    writeFileSync(
      join(host, "resolver.js"),
      "export const localResolver = () => {};",
    );
    writeFileSync(join(client, "index.js"), "export {};");
    const module = join(root, "packages/ai-host/dist");
    mkdirSync(module, { recursive: true });
    writeFileSync(
      join(module, "index.js"),
      "export const createHost = () => { throw Error('no model probes'); };",
    );
    const store = join(root, "packages/ai-store-sqlite/dist");
    mkdirSync(store, { recursive: true });
    writeFileSync(
      join(store, "index.js"),
      `
import { writeFileSync } from 'node:fs';
import fs from 'node:fs/promises';
import { syncBuiltinESMExports } from 'node:module';
import { fileURLToPath } from 'node:url';
const input = fileURLToPath(new URL('../../../crates/sample/input.rs', import.meta.url));
export function openSqliteStore() {
  return { ok: true, value: { close: async () => {
    if (process.env.CONNECTION_TEST_MODE === 'drift') writeFileSync(input, 'changed');
    if (process.env.CONNECTION_TEST_MODE === 'publish-drift') {
      const write = fs.writeFile;
      let changed = false;
      fs.writeFile = async (path, ...args) => {
        await write(path, ...args);
        if (!changed && String(path).endsWith('connection-sources-preflight.json')) {
          changed = true; writeFileSync(input, 'changed after publication');
        }
      };
      syncBuiltinESMExports();
    }
    if (process.env.CONNECTION_TEST_MODE === 'cancel') {
      process.kill(process.pid, 'SIGTERM'); await new Promise(setImmediate);
    }
    return { ok: true };
  } } };
}
`,
    );
    const env = { ...process.env, CONNECTION_TEST_MODE: mode };
    delete env[contextKey];
    delete env._AGENT_BUILD_LEASE;
    const preparation = new Preparation(root, { env });
    t.after(() => preparation.close());
    mkdirSync(join(root, ".local-ci-runs"));
    preparation.directory = mkdtempSync(
      join(root, ".local-ci-runs/preparation-"),
    );
    for (const [name, path] of [
      ["ai-host-app", host],
      ["ai-client", client],
    ]) {
      preparation.nodes.add(`@rss-mdm-agent/${name}`);
      preparation.record(`node:@rss-mdm-agent/${name}`, path);
    }
    const worker = join(preparation.directory, "worker");
    mkdirSync(join(worker, "bin"), { recursive: true });
    cpSync(process.execPath, join(worker, "bin/node"));
    preparation.record("worker", worker);
    preparation.record("privateWorker", worker);
    const output = join(
      root,
      ".local-ci-runs/connection-sources-preflight.json",
    );
    writeFileSync(output, '{"status":"passed","old":true}');
    if (mode === "missing") rmSync(client, { recursive: true });
    const preload = `
import fs from 'node:fs/promises';
import { syncBuiltinESMExports } from 'node:module';
const remove = fs.rm;
fs.rm = async (path, ...args) => {
  if (String(path).endsWith('connection-sources-preflight.json')) {
    process.kill(process.pid, 'SIGTERM'); await new Promise(setImmediate);
  }
  return remove(path, ...args);
};
syncBuiltinESMExports();
`;
    const args = ["scripts/check-connection-sources.mjs", "--preflight"];
    if (mode === "retirement-cancel")
      args.unshift(
        "--import",
        `data:text/javascript;base64,${Buffer.from(preload).toString("base64")}`,
      );
    const result = spawnSync(process.execPath, args, {
      cwd: root,
      env: preparation.env,
      encoding: "utf8",
      timeout: 10000,
    });
    const receipt = JSON.parse(readFileSync(output, "utf8"));
    assert.equal(result.status, mode === "success" ? 0 : 1, result.stderr);
    assert.equal(
      receipt.status,
      mode === "success"
        ? "passed"
        : mode.includes("cancel")
          ? "cancelled"
          : "failed",
    );
    assert.equal(receipt.old, undefined);
    if (mode === "success") assert.equal(receipt.sqliteReady, true);
    if (mode.includes("cancel")) assert.equal(receipt.signal, "SIGTERM");
    if (mode !== "success") assert.ok(receipt.failure);
  });
