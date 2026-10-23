import assert from "node:assert/strict";
import { test } from "node:test";
import {
  mkdtempSync,
  chmodSync,
  symlinkSync,
  writeFileSync,
  statSync,
  realpathSync,
  rmSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, toNamespacedPath } from "node:path";
import { execFileSync } from "node:child_process";
import { privateDirectory } from "../../../packages/platform-private-storage/dist/index.js";
import { sdkOptions } from "../../../packages/ai-adapters/claude/dist/configuration.js";
const resolved = (directory) => ({
  configuration: { workingDirectory: directory },
  configurationDirectory: directory,

  authentication: {
    type: "custom_api",
    apiUrl: "https://example.invalid",
    credential: { type: "api_key", value: "fixture" },
  },
});
test("Claude persistence accepts only private owned real directories before SDK launch", async (t) => {
  const directory = mkdtempSync(join(tmpdir(), "rss-claude-private-"));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const shared = join(directory, "shared"),
    link = join(directory, "link"),
    file = join(directory, "file");
  await privateDirectory(shared);
  if (process.platform === "win32")
    execFileSync("icacls.exe", [shared, "/grant", "*S-1-1-0:(RX)"]);
  else chmodSync(shared, 0o755);
  await assert.rejects(() => sdkOptions(resolved(shared)), /configuration/);
  if (process.platform === "win32")
    execFileSync("icacls.exe", [shared, "/remove:g", "*S-1-1-0"]);
  else chmodSync(shared, 0o700);
  symlinkSync(shared, link, process.platform === "win32" ? "junction" : "dir");
  await assert.rejects(() => sdkOptions(resolved(link)), /configuration/);
  writeFileSync(file, "", { mode: 0o600 });
  await assert.rejects(() => sdkOptions(resolved(file)), /configuration/);
  const fresh = join(directory, "fresh");
  assert.equal(
    (await sdkOptions(resolved(fresh))).env.CLAUDE_CONFIG_DIR,
    realpathSync(fresh),
  );
  if (process.platform !== "win32")
    assert.equal(statSync(fresh).mode & 0o777, 0o700);
  if (process.platform === "win32") {
    const native = toNamespacedPath(join(directory, "namespace"));
    assert.equal(
      (await sdkOptions(resolved(native))).env.CLAUDE_CONFIG_DIR,
      realpathSync.native(native),
    );
  }
  assert.equal(
    (await sdkOptions(resolved(shared))).env.CLAUDE_CONFIG_DIR,
    realpathSync(shared),
  );
});

test("existing Claude configuration passes directly to the SDK with user settings and sealed execution capabilities", async (t) => {
  const directory = mkdtempSync(join(tmpdir(), "rss-claude-login-"));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const value = await sdkOptions({
    ...resolved(directory),
    authentication: { type: "existing_config", directory },
    verification: true,
  });
  assert.equal(value.env.CLAUDE_CONFIG_DIR, directory);
  assert.deepEqual(value.settingSources, ["user"]);
  assert.equal(value.settings.disableAllHooks, true);
  assert.equal(value.strictMcpConfig, true);
  assert.equal(value.persistSession, false);
  assert.equal(value.env.ANTHROPIC_API_KEY, undefined);
});
