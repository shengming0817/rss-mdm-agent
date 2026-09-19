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
import { readConfiguration } from "../../apps/ai-host/dist/configuration.js";
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
        accountRef: "a",
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
  await writeFile(path, JSON.stringify(base), { mode: 0o600 });
  assert.equal(
    (await readConfiguration(path)).connection.apiUrl,
    base.connection.apiUrl,
  );
  await writeFile(base.socketPath, "preserve this file");
  await assert.rejects(startLocalApp(path), /socket path is not a socket/);
  assert.equal(await readFile(base.socketPath, "utf8"), "preserve this file");
});
