import test from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, mkdir, writeFile, rm, readFile } from "node:fs/promises";
import { tmpdir, homedir } from "node:os";
import { join } from "node:path";
import { resolveConnection } from "../../apps/ai-host/dist/connection.js";
const connection = (provider) => ({
  schemaVersion: 5,
  kind: "connection",
  connectionId: "one",
  configRevision: 1,
  name: "Local",
  provider,
  source: { type: "existing_config" },
  status: "ready",
  profile: "conversation",
});
test("existing config delegates missing, malformed and credential files to the official tools without inspecting them", async (t) => {
  const root = await mkdtemp(join(tmpdir(), "rss-official-config-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const source = join(root, "user");
  await mkdir(source);
  for (const file of [
    "auth.json",
    "config.toml",
    "settings.json",
    ".credentials.json",
  ])
    await writeFile(join(source, file), "deliberately not parseable\n");
  for (const provider of ["codex", "claude"]) {
    const row = connection(provider);
    row.source = { type: "existing_config", directory: source };
    const resolved = resolveConnection({ connection: row });
    assert.equal(resolved.model, undefined);
    assert.equal(resolved[provider].type, "existing_config");
    assert.equal(resolved[provider].directory, source);
    assert.equal("credential" in resolved, false);
    assert.equal("refresh" in resolved[provider], false);
    delete row.source.directory;
    assert.equal(
      resolveConnection({ connection: row })[provider].directory,
      join(homedir(), `.${provider}`),
    );
  }
  assert.equal(
    await readFile(join(source, "auth.json"), "utf8"),
    "deliberately not parseable\n",
  );
});
test("custom API activation needs the memory-only secret and DeepSeek has no existing-config mode", () => {
  for (const provider of ["codex", "claude", "deepseek"]) {
    const row = connection(provider);
    row.source = {
      type: "custom_api",
      apiUrl: "https://example.invalid/v1",
      model: "selected",
    };
    assert.throws(() => resolveConnection({ connection: row }));
    const resolved = resolveConnection({
      connection: row,
      secret: "test-only-key",
    });
    assert.equal(resolved.apiKey, "test-only-key");
    assert.equal(resolved.model, "selected");
    assert.equal(JSON.stringify(row).includes("test-only-key"), false);
  }
  assert.throws(() =>
    resolveConnection({ connection: connection("deepseek") }),
  );
});
