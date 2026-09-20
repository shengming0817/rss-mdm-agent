import { test } from "node:test";
import assert from "node:assert/strict";
import {
  mkdtemp,
  realpath,
  writeFile,
  mkdir,
  chmod,
  rm,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { resolveConnection } from "../../apps/ai-host/dist/connection.js";
import { readConfiguration } from "../../apps/ai-host/dist/configuration.js";
async function setup(t, provider) {
  const root = await realpath(await mkdtemp(join(tmpdir(), "rss-connection-")));
  t.after(() => rm(root, { recursive: true, force: true }));
  const user = join(root, "user");
  await mkdir(user, { mode: 0o755 });
  const local = {
    databasePath: join(root, "ai.sqlite"),
    socketPath: join(root, "ai.sock"),
    nativeDirectory: join(root, "native"),
    workingDirectory: root,
    caller: { tenantId: "t", principalId: "p", authorityId: "a" },
    session: {
      provider,
      config: { id: "local", revision: "1" },
      accountRef: "account",
      profile: "conversation",
    },
    connection: { source: "existing_user_config", directory: user },
  };
  return {
    root,
    user,
    local,
    write: async (name, value) =>
      writeFile(join(user, name), value, { mode: 0o600 }),
  };
}
test("all engines accept an explicit non-official endpoint and private credential", async (t) => {
  for (const provider of ["claude", "codex", "deepseek"]) {
    const { root, local } = await setup(t, provider);
    const path = join(root, "configuration.json"),
      credentialPath = join(root, "key");
    await writeFile(credentialPath, "test-private-key", { mode: 0o600 });
    local.connection = {
      source: "custom_endpoint",
      apiUrl: "https://models.example.test/api",
      credentialPath,
      credentialType: "api_key",
      model: "chosen-model",
    };
    await writeFile(path, JSON.stringify(local), { mode: 0o600 });
    const connection = await resolveConnection(await readConfiguration(path));
    assert.equal(connection.apiUrl, "https://models.example.test/api");
    assert.equal(connection.model, "chosen-model");
  }
});
test("existing Codex auth uses external tokens and refreshes only the pinned account", async (t) => {
  const { write, local, user } = await setup(t, "codex");
  await write(
    "config.toml",
    'model = "chosen-model"\nsandbox_mode = "danger-full-access"\n',
  );
  const auth = (token) =>
    JSON.stringify({
      auth_mode: "chatgpt",
      tokens: {
        access_token: token,
        account_id: "account-a",
        refresh_token: "must-not-copy",
      },
    });
  await write("auth.json", auth("first-token"));
  const value = await resolveConnection(local);
  assert.equal(value.codex.type, "chatgpt_tokens");
  assert.equal(value.codex.accessToken, "first-token");
  assert.equal(value.codex.apiUrl, undefined);
  assert.equal(JSON.stringify(value).includes("must-not-copy"), false);
  await write("auth.json", auth("second-token"));
  assert.equal(
    (
      await value.codex.refresh({
        timeoutMs: 1000,
        signal: AbortSignal.timeout(1000),
      })
    ).accessToken,
    "second-token",
  );
  await write(
    "auth.json",
    auth("third-token").replace("account-a", "account-b"),
  );
  await assert.rejects(
    value.codex.refresh({ timeoutMs: 1000, signal: AbortSignal.timeout(1000) }),
  );
  await chmod(join(user, "auth.json"), 0o644);
  await assert.rejects(resolveConnection(local));
});
test("Codex profile custom provider resolves explicit endpoint and cannot silently fall back", async (t) => {
  const { write, local } = await setup(t, "codex");
  local.connection.profile = "chosen";
  await write(
    "auth.json",
    JSON.stringify({ OPENAI_API_KEY: "private-api-key" }),
  );
  await write(
    "config.toml",
    'model="root-model"\n[profiles.chosen]\nmodel="profile-model"\nmodel_provider="custom"\n[model_providers.custom]\nbase_url="https://custom.example.test/v1"\n',
  );
  const value = await resolveConnection(local);
  assert.equal(value.model, "profile-model");
  assert.equal(value.codex.apiUrl, "https://custom.example.test/v1");
  await write(
    "config.toml",
    'model="root-model"\n[profiles.chosen]\nmodel_provider="missing"\n',
  );
  await assert.rejects(resolveConnection(local));
});
test("Claude and DSH read only connection fields from existing settings", async (t) => {
  const claude = await setup(t, "claude");
  await claude.write(
    "settings.json",
    JSON.stringify({
      model: "c-model",
      env: {
        ANTHROPIC_BASE_URL: "https://c.example.test",
        CLAUDE_CODE_OAUTH_TOKEN: "oauth-test",
      },
      permissions: { defaultMode: "bypassPermissions" },
      mcpServers: { bad: { command: "forbidden" } },
    }),
  );
  const c = await resolveConnection(claude.local);
  assert.equal(c.apiUrl, "https://c.example.test");
  assert.deepEqual(c.credential, { type: "oauth_token", value: "oauth-test" });
  assert.equal(JSON.stringify(c).includes("bypassPermissions"), false);
  const dsh = await setup(t, "deepseek");
  await dsh.write(
    ".credentials.yaml",
    "version: 1\nrefs:\n  DEEPSEEK_API_KEY: private-dsh-key\n",
  );
  await dsh.write("settings.yaml", "agent-default-model:\n  model: d-model\n");
  await mkdir(join(dsh.user, "profiles", "chosen"), {
    recursive: true,
    mode: 0o700,
  });
  dsh.local.connection.profile = "chosen";
  await dsh.write(
    "profiles/chosen/cordis.yml",
    '- name: "@deepseek-ai/dsh-llm-deepseek"\n  config:\n    baseURL: https://d.example.test\n- name: forbidden-plugin\n  config:\n    command: forbidden\n',
  );
  const d = await resolveConnection(dsh.local);
  assert.equal(d.apiUrl, "https://d.example.test");
  assert.equal(d.model, "d-model");
  assert.equal(JSON.stringify(d).includes("forbidden"), false);
});
test("inline credentials require private settings and connection lineage rejects identity drift", async (t) => {
  const { bindConnection } = await import(
    "../../apps/ai-host/dist/connection.js"
  );
  for (const provider of ["claude", "codex"]) {
    const f = await setup(t, provider);
    const filename = provider === "claude" ? "settings.json" : "config.toml";
    await f.write(
      filename,
      provider === "claude"
        ? JSON.stringify({ env: { ANTHROPIC_API_KEY: "inline-secret" } })
        : 'model="model"\nmodel_provider="custom"\n[model_providers.custom]\nbase_url="https://models.example.test/v1"\nexperimental_bearer_token="inline-secret"\n',
    );
    if (provider === "codex") await f.write("auth.json", "{}");
    await chmod(join(f.user, filename), 0o644);
    await assert.rejects(resolveConnection(f.local));
    await chmod(join(f.user, filename), 0o600);
    const c = await resolveConnection(f.local);
    await bindConnection(f.local, c);
    await bindConnection(f.local, c);
    for (const changed of [
      { ...c, apiUrl: "https://different.example.test/v1" },
      { ...c, model: "other-model" },
      { ...c, credential: { type: "api_key", value: "other-account" } },
    ])
      await assert.rejects(bindConnection(f.local, changed));
  }
  const f = await setup(t, "codex");
  await f.write("config.toml", 'model="chosen"\n');
  const auth = (token, account) =>
    JSON.stringify({
      auth_mode: "chatgpt",
      tokens: { access_token: token, account_id: account },
    });
  await f.write("auth.json", auth("old-token", "same-account"));
  await bindConnection(f.local, await resolveConnection(f.local));
  await f.write("auth.json", auth("new-token", "same-account"));
  await bindConnection(f.local, await resolveConnection(f.local));
  await f.write("auth.json", auth("new-token", "different-account"));
  await assert.rejects(
    bindConnection(f.local, await resolveConnection(f.local)),
  );
});

test("existing Codex configuration may omit model but rejects an invalid explicit model", async (t) => {
  const { write, local } = await setup(t, "codex");
  await write("auth.json", JSON.stringify({ OPENAI_API_KEY: "fixture-key" }));
  await write("config.toml", "");
  assert.equal((await resolveConnection(local)).model, undefined);
  for (const value of ['""', '"  "', "17"]) {
    await write("config.toml", `model=${value}\n`);
    await assert.rejects(resolveConnection(local));
  }
});
