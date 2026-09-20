import { test } from "node:test";
import assert from "node:assert/strict";
import {
  mkdtemp,
  realpath,
  writeFile,
  mkdir,
  chmod,
  rm,
  readFile,
} from "node:fs/promises";
import { createServer } from "node:net";
import { once } from "node:events";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { resolveConnection } from "../../apps/ai-host/dist/connection.js";
async function setup(t, provider) {
  const root = await realpath(await mkdtemp(join(tmpdir(), "rss-connection-")));
  const user = join(root, "user");
  await mkdir(user, { mode: 0o755 });
  const local = {
    version: 1,
    databasePath: join(root, "ai.sqlite"),
    socketPath: join(root, "ai.sock"),
    credentialSocket: join(root, "credentials.sock"),
    usersPath: join(root, "users.json"),
    nativeDirectory: join(root, "native"),
    workingDirectory: root,
  };
  const snapshot = {
    generation: "fixture-generation",
    local,
    namespace: {
      tenantId: "t",
      principalId: "alice",
      authorityId: "a",
      sessionId: "s",
    },
    connection: {
      schemaVersion: 5,
      kind: "connection",
      connectionId: "connection",
      name: "Named connection",
      provider,
      configRevision: 1,
      credentialRevision: 1,
      accountRef: "opaque-account",
      credentialRef: "native-ref",
      profile: "conversation",
      status: "ready",
      source: { type: "existing_api", directory: user },
    },
  };
  let read = async () => ({ value: "fixture-key" });
  // A scripted native broker verifies the private protocol, not real Keychain acceptance.
  const server = createServer((socket) => {
    let data = "";
    socket.on("data", (chunk) => {
      data += chunk;
      if (!data.includes("\n")) return;
      socket.pause();
      void read(JSON.parse(data)).then(
        (value) => socket.end(JSON.stringify({ ok: true, value }) + "\n"),
        () => socket.end('{"ok":false}\n'),
      );
    });
  });
  server.listen(local.credentialSocket);
  await once(server, "listening");
  t.after(async () => {
    await new Promise((resolve) => server.close(resolve));
    await rm(root, { recursive: true, force: true });
  });
  return {
    root,
    user,
    snapshot,
    write: (name, value) => writeFile(join(user, name), value, { mode: 0o600 }),
    broker: (fn) => {
      read = fn;
    },
  };
}
test("custom endpoints resolve credentials only through the native user-scoped reference", async (t) => {
  for (const provider of ["claude", "codex", "deepseek"]) {
    const f = await setup(t, provider);
    f.snapshot.connection.source = {
      type: "custom_api",
      apiUrl: "https://models.example.test/api",
      model: "chosen-model",
      credentialType: "api_key",
    };
    f.broker(async (request) => {
      assert.deepEqual(request, {
        type: "credential",
        userId: "alice",
        generation: "fixture-generation",
        credentialRef: "native-ref",
      });
      return { value: "fixture-key" };
    });
    const value = await resolveConnection(f.snapshot);
    assert.equal(value.apiUrl, "https://models.example.test/api");
    assert.equal(value.model, "chosen-model");
    assert.equal(value.credential.value, "fixture-key");
    assert.equal(JSON.stringify(f.snapshot).includes("fixture-key"), false);
  }
});
test("Codex login pins source/account, rereads rotation, and rejects unchanged or different-account tokens", async (t) => {
  const f = await setup(t, "codex");
  f.snapshot.connection.source.type = "existing_login";
  await f.write(
    "config.toml",
    'model="chosen-model"\ncli_auth_credentials_store="auto"\nsandbox_mode="danger-full-access"\n',
  );
  let token = "first-token",
    account = "account-a";
  f.broker(async (request) => {
    assert.equal(request.storage, "auto");
    assert.equal(request.directory, f.user);
    return { accessToken: token, accountId: account };
  });
  const value = await resolveConnection(f.snapshot),
    budget = { timeoutMs: 1000, signal: AbortSignal.timeout(1000) };
  assert.equal(value.codex.type, "chatgpt_tokens");
  assert.equal(value.codex.accessToken, token);
  await assert.rejects(value.codex.refresh(budget), {
    code: "authentication_required",
  });
  token = "second-token";
  assert.equal((await value.codex.refresh(budget)).accessToken, token);
  assert.equal((await resolveConnection(f.snapshot)).codex.accessToken, token);
  account = "account-b";
  token = "third-token";
  await assert.rejects(value.codex.refresh(budget), {
    code: "authentication_required",
  });
  await assert.rejects(resolveConnection(f.snapshot), {
    code: "authentication_required",
  });
});
test("Codex profile resolves explicit API settings without fallback to login or a missing provider", async (t) => {
  const f = await setup(t, "codex");
  f.snapshot.connection.source.profile = "chosen";
  await f.write("auth.json", JSON.stringify({ OPENAI_API_KEY: "fixture-key" }));
  await f.write(
    "config.toml",
    'model="root"\n[profiles.chosen]\nmodel="selected"\nmodel_provider="custom"\n[model_providers.custom]\nbase_url="https://custom.example.test/v1"\n',
  );
  const value = await resolveConnection(f.snapshot);
  assert.equal(value.model, "selected");
  assert.equal(value.codex.apiUrl, "https://custom.example.test/v1");
  await f.write("config.toml", '[profiles.chosen]\nmodel_provider="missing"\n');
  await assert.rejects(resolveConnection(f.snapshot));
  await chmod(join(f.user, "auth.json"), 0o644);
  await assert.rejects(resolveConnection(f.snapshot));
});
test("Claude API imports only explicit API fields; CLI OAuth environment is never a login substitute", async (t) => {
  const f = await setup(t, "claude");
  await f.write(
    "settings.json",
    JSON.stringify({
      model: "chosen",
      env: {
        ANTHROPIC_BASE_URL: "https://c.example.test",
        ANTHROPIC_AUTH_TOKEN: "fixture-token",
      },
      permissions: { defaultMode: "bypassPermissions" },
      mcpServers: { bad: { command: "forbidden" } },
    }),
  );
  const value = await resolveConnection(f.snapshot);
  assert.equal(value.apiUrl, "https://c.example.test");
  assert.deepEqual(value.credential, {
    type: "auth_token",
    value: "fixture-token",
  });
  assert.equal(JSON.stringify(value).includes("bypassPermissions"), false);
  await chmod(join(f.user, "settings.json"), 0o644);
  await assert.rejects(resolveConnection(f.snapshot));
  await chmod(join(f.user, "settings.json"), 0o600);
  await f.write(
    "settings.json",
    JSON.stringify({ env: { CLAUDE_CODE_OAUTH_TOKEN: "must-not-import" } }),
  );
  await assert.rejects(resolveConnection(f.snapshot), {
    code: "authentication_required",
  });
  f.snapshot.connection.source.type = "existing_login";
  await assert.rejects(resolveConnection(f.snapshot), {
    code: "unsupported_capability",
  });
});
test("missing model may use Codex defaults, while explicit invalid models are rejected", async (t) => {
  const f = await setup(t, "codex");
  await f.write("auth.json", JSON.stringify({ OPENAI_API_KEY: "fixture-key" }));
  await f.write("config.toml", "");
  assert.equal((await resolveConnection(f.snapshot)).model, undefined);
  for (const value of ['""', '"  "', "17"]) {
    await f.write("config.toml", `model=${value}\n`);
    await assert.rejects(resolveConnection(f.snapshot));
  }
});

test("credential retirement retains unresolved receipt revisions and rejects an inactive native user", async (t) => {
  const f = await setup(t, "codex");
  const { credentialLifecycle } = await import(
    "../../apps/ai-host/dist/credential-lifecycle.js"
  );
  const { openSqliteStore } = await import(
    "../../packages/ai-store-sqlite/dist/index.js"
  );
  const { fixtureSession, unwrap } = await import(
    "../../packages/ai-contract/dist/testing/index.js"
  );
  const { activeStage } = await import(
    "../../packages/ai-contract/dist/index.js"
  );
  const store = unwrap(
    openSqliteStore({ path: f.snapshot.local.databasePath, mode: "create" }),
  );
  t.after(() =>
    store.close({ timeoutMs: 1000, signal: new AbortController().signal }),
  );
  const caller = {
    tenantId: "test-users",
    principalId: "alice",
    authorityId: "desktop-fixture",
  };
  const user = {
    schemaVersion: 5,
    kind: "testUser",
    userId: "alice",
    displayName: "Alice",
    nameKey: "alice",
  };
  const page = {
    schemaVersion: 5,
    kind: "testUserPage",
    users: [user],
    current: {
      schemaVersion: 5,
      kind: "userContext",
      user,
      generation: "current-generation",
    },
  };
  await writeFile(f.snapshot.local.usersPath, JSON.stringify(page), {
    mode: 0o600,
  });
  const session = fixtureSession();
  session.namespace = { ...caller, sessionId: "historical" };
  const phase = activeStage(session);
  const old = {
    ...f.snapshot.connection,
    connectionId: phase.connectionId,
    source: {
      type: "custom_api",
      apiUrl: "https://example.invalid",
      model: "model",
    },
    credentialRef: "old-key",
  };
  unwrap(await store.saveConnection(caller, old, null));
  unwrap(await store.create(session));
  const retention = { retryWindowMs: 60000, receiptWindowMs: 86400000 };
  unwrap(
    await store.accept({
      namespace: session.namespace,
      command: {
        schemaVersion: 5,
        kind: "command",
        commandId: "pending",
        sessionId: "historical",
        expiresAtMs: Date.now() + 60000,
        input: { type: "prompt", policy: "queue_next", text: "pending" },
      },
      expectedRevision: 0,
      expectedGeneration: phase.binding.generation,
      nowMs: Date.now(),
      retention,
      eventId: "accept",
    }),
  );
  const next = {
    ...old,
    configRevision: 2,
    credentialRevision: 2,
    credentialRef: "new-key",
  };
  unwrap(await store.saveConnection(caller, next, 1));
  const requests = [];
  f.broker(async (request) => {
    requests.push(request);
    return null;
  });
  const lifecycle = credentialLifecycle(f.snapshot.local, store);
  await lifecycle.activate(caller, next);
  assert.deepEqual(requests.at(-1), {
    type: "activate",
    credentialRef: "new-key",
    userId: "alice",
    generation: "current-generation",
  });
  await lifecycle.collect(caller);
  assert.deepEqual(requests.at(-1).keep.sort(), ["new-key", "old-key"]);
  const head = unwrap(await store.session(session.namespace));
  unwrap(
    await store.suspend({
      namespace: head.namespace,
      expectedRevision: head.revision,
      expectedGeneration: phase.binding.generation,
      nowMs: Date.now(),
      retention,
      eventId: "suspend",
    }),
  );
  await lifecycle.collect(caller);
  assert.deepEqual(requests.at(-1).keep, ["new-key"]);
  unwrap(
    await store.saveConnection(
      caller,
      { ...next, configRevision: 3, status: "deleted" },
      2,
    ),
  );
  await lifecycle.collect(caller);
  assert.deepEqual(requests.at(-1).keep, []);
  const count = requests.length;
  await assert.rejects(
    lifecycle.activate({ ...caller, principalId: "bob" }, next),
    { code: "authentication_required" },
  );
  assert.equal(requests.length, count);
});
