import assert from "node:assert/strict";
import { test } from "node:test";
import { spawnSync } from "node:child_process";
import {
  mkdtemp,
  writeFile,
  symlink,
  chmod,
  rm,
  readFile,
} from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";
import { readPrivateFile } from "../../apps/ai-host/dist/private-file.js";
import {
  endpoint,
  readConfiguration,
} from "../../apps/ai-host/dist/configuration.js";
import { startLocalApp } from "../../apps/ai-host/dist/index.js";
test("private configuration and credentials require bounded owned files in private directories", async (t) => {
  const directory = await mkdtemp(join(tmpdir(), "rss-private-file-"));
  t.after(() => rm(directory, { recursive: true, force: true }));
  const file = join(directory, "secret"),
    link = join(directory, "link");
  await writeFile(file, "secret", { mode: 0o600 });
  assert.equal(await readPrivateFile(file, 6), "secret");
  await assert.rejects(readPrivateFile(file, 5));
  await symlink(file, link);
  await assert.rejects(readPrivateFile(link, 64));
  const fifo = join(directory, "fifo");
  assert.equal(spawnSync("/usr/bin/mkfifo", [fifo]).status, 0);
  await chmod(fifo, 0o600);
  const moduleUrl = new URL(
    "../../apps/ai-host/dist/private-file.js",
    import.meta.url,
  ).href;
  const rejected = spawnSync(
    process.execPath,
    [
      "--input-type=module",
      "-e",
      `import {readPrivateFile} from ${JSON.stringify(moduleUrl)}; try {await readPrivateFile(process.argv[1],64);process.exitCode=2;}catch {process.exitCode=0;}`,
      fifo,
    ],
    { timeout: 1000 },
  );
  assert.equal(
    rejected.status,
    0,
    "non-regular files must be rejected without blocking open",
  );
  await chmod(file, 0o644);
  await assert.rejects(readPrivateFile(file, 64));
  await chmod(file, 0o600);
  await chmod(directory, 0o755);
  await assert.rejects(readPrivateFile(file, 64));
});
test("custom HTTPS endpoints reject literal private and link-local destinations", () => {
  for (const value of [
    "https://127.0.0.1/v1",
    "https://10.0.0.1/v1",
    "https://169.254.169.254/latest/meta-data",
    "https://[::1]/v1",
    "https://[fe80::1]/v1",
  ])
    assert.throws(() => endpoint(value), { code: "configuration_invalid" });
  assert.equal(
    endpoint("https://api.example.test/v1/"),
    "https://api.example.test/v1",
  );
});
test("invalid local configuration fails before listening and emits only a closed diagnostic", async (t) => {
  const directory = await mkdtemp(join(tmpdir(), "rss-invalid-config-"));
  t.after(() => rm(directory, { recursive: true, force: true }));
  const path = join(directory, "configuration.json"),
    base = {
      databasePath: join(directory, "host.sqlite"),
      socketPath: join(directory, "host.sock"),
      caller: { tenantId: "t", principalId: "p", authorityId: "a" },
      session: {
        provider: "claude",
        config: { id: "c", revision: "1" },

        profile: "conversation",
      },
      workingDirectory: directory,
      nativeDirectory: directory,
      connection: {
        source: "custom_endpoint",
        model: "fixture-model",
        credentialPath: join(directory, "credential"),
        credentialType: "api_key",
        apiUrl: "https://api.example.test",
      },
    };
  for (const connection of [
    { ...base.connection, apiUrl: undefined },
    { ...base.connection, model: 42 },
    { ...base.connection, apiUrl: "https://secret-value@example.test" },
    { ...base.connection, apiUrl: "http://remote.example.test" },
  ]) {
    await writeFile(path, JSON.stringify({ ...base, connection }), {
      mode: 0o600,
    });
    await assert.rejects(
      readConfiguration(path),
      (error) => error.code === "configuration_invalid",
    );
    await assert.rejects(
      startLocalApp(path),
      (error) => error.code === "configuration_invalid",
    );
  }
  const cli = spawnSync(
    process.execPath,
    [
      fileURLToPath(new URL("../../apps/ai-host/dist/cli.js", import.meta.url)),
      path,
    ],
    { encoding: "utf8", timeout: 3000 },
  );
  assert.equal(cli.status, 1);
  assert.match(cli.stderr, /configuration_invalid/);
  assert.doesNotMatch(cli.stderr, /remote\.example\.test|secret-value/);
  const current = {
    version: 1,
    databasePath: base.databasePath,
    nativeDirectory: directory,
    workingDirectory: directory,
  };
  await writeFile(path, JSON.stringify(current), { mode: 0o600 });
  assert.deepEqual(await readConfiguration(path), current);
  await writeFile(path, JSON.stringify({ ...current, caller: base.caller }), {
    mode: 0o600,
  });
  await assert.rejects(readConfiguration(path), {
    code: "configuration_invalid",
  });
});

test("a corrupt existing store retains its closed startup category without exposing database content", async () => {
  const { mkdtemp, writeFile, rm } = await import("node:fs/promises");
  const { tmpdir } = await import("node:os");
  const { join } = await import("node:path");
  const { spawnSync } = await import("node:child_process");
  const root = await mkdtemp(join(tmpdir(), "rss-corrupt-start-"));
  try {
    const db = join(root, "ai.sqlite"),
      config = join(root, "host.json");
    await writeFile(db, "CANARY_SECRET_DB", { mode: 0o600 });
    await writeFile(
      config,
      JSON.stringify({
        version: 1,
        databasePath: db,
        nativeDirectory: join(root, "native"),
        workingDirectory: root,
      }),
      { mode: 0o600 },
    );
    const result = spawnSync(
      process.execPath,
      [
        new URL("../../apps/ai-host/dist/cli.js", import.meta.url).pathname,
        config,
      ],
      { encoding: "utf8", timeout: 10000 },
    );
    assert.equal(result.status, 1);
    assert.ok(
      result.stderr.split("\n").some((line) => {
        try {
          return (
            JSON.parse(line).kind === "hostProcessDiagnostic" &&
            JSON.parse(line).code === "storage_corrupt"
          );
        } catch {
          return false;
        }
      }),
    );
    assert.equal(result.stderr.includes("CANARY_SECRET_DB"), false);
    assert.equal(result.stderr.includes(root), false);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
