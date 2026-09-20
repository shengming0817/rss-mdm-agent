import assert from "node:assert/strict";
import { test } from "node:test";
import {
  mkdtempSync,
  mkdirSync,
  chmodSync,
  symlinkSync,
  writeFileSync,
  statSync,
  realpathSync,
  rmSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
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
test("Claude persistence accepts only private owned real directories before SDK launch", (t) => {
  const directory = mkdtempSync(join(tmpdir(), "rss-claude-private-"));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const shared = join(directory, "shared"),
    link = join(directory, "link"),
    file = join(directory, "file");
  mkdirSync(shared, { mode: 0o755 });
  chmodSync(shared, 0o755);
  assert.throws(() => sdkOptions(resolved(shared)), /configuration/);
  chmodSync(shared, 0o700);
  symlinkSync(shared, link);
  assert.throws(() => sdkOptions(resolved(link)), /configuration/);
  writeFileSync(file, "", { mode: 0o600 });
  assert.throws(() => sdkOptions(resolved(file)), /configuration/);
  const fresh = join(directory, "fresh");
  assert.equal(
    sdkOptions(resolved(fresh)).env.CLAUDE_CONFIG_DIR,
    realpathSync(fresh),
  );
  assert.equal(statSync(fresh).mode & 0o777, 0o700);
  assert.equal(
    sdkOptions(resolved(shared)).env.CLAUDE_CONFIG_DIR,
    realpathSync(shared),
  );
});

test("existing Claude configuration passes directly to the SDK with user settings and sealed execution capabilities", (t) => {
  const directory = mkdtempSync(join(tmpdir(), "rss-claude-login-"));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const value = sdkOptions({
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
