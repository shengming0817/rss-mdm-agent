import assert from "node:assert/strict";
import { spawn, execFileSync } from "node:child_process";
import { once } from "node:events";
import { mkdtemp, mkdir, writeFile, rm, lstat } from "node:fs/promises";
import { connect } from "node:net";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { Readable, Writable } from "node:stream";
import {
  RuntimeClient,
  ndJsonStream,
} from "../packages/ai-client/dist/index.js";
import { createModelServer } from "../tests/ai-adapters/claude/model-fixture.mjs";

const executable = resolve(process.argv[2]);
const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
async function until(check) {
  const deadline = Date.now() + 15000;
  while (Date.now() < deadline) {
    if (await check()) return;
    await pause(20);
  }
  assert.fail("runtime lifecycle deadline");
}
const groupEmpty = (pgid) => {
  try {
    process.kill(-pgid, 0);
    return false;
  } catch (error) {
    return error.code === "ESRCH";
  }
};
for (const signal of ["SIGTERM", "SIGINT"]) {
  const directory = await mkdtemp(join(tmpdir(), "rss bundled host-"));
  const requests = [],
    server = createModelServer(
      [[{ type: "text", text: "bundled runtime" }]],
      requests,
    );
  let child, socket, client, database;
  let groups = [],
    stderr = "";
  try {
    await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
    const configurationDirectory = join(directory, "claude");
    await mkdir(configurationDirectory, { mode: 0o700 });
    const credentialPath = join(directory, "credential");
    await writeFile(credentialPath, "fixture-only-key", { mode: 0o600 });
    const configuration = {
      databasePath: join(directory, "host.sqlite"),
      socketPath: join(directory, "host.sock"),
      caller: { tenantId: "t", principalId: "p", authorityId: "a" },
      session: {
        provider: "claude",
        config: { id: "bundled", revision: "1" },
        accountRef: "fixture",
        profile: "conversation",
      },
      workingDirectory: directory,
      nativeDirectory: configurationDirectory,
      connection: {
        source: "custom_endpoint",
        credentialPath,
        credentialType: "api_key",
        apiUrl: `http://127.0.0.1:${server.address().port}`,
        model: "fixture-model",
      },
    };
    const configurationPath = join(directory, "configuration.json");
    await writeFile(configurationPath, JSON.stringify(configuration), {
      mode: 0o600,
    });
    child = spawn(executable, [configurationPath], {
      cwd: directory,
      stdio: ["ignore", "ignore", "pipe"],
      detached: true,
      env: { PATH: "/usr/bin:/bin", HOME: directory, TMPDIR: tmpdir() },
    });
    child.stderr.on("data", (chunk) => {
      stderr = (stderr + chunk).slice(-16384);
    });
    const exited = once(child, "exit");
    await until(async () => {
      assert.equal(child.exitCode, null, stderr);
      assert.equal(child.signalCode, null, stderr);
      const stat = await lstat(configuration.socketPath).catch(() => undefined);
      return stat?.isSocket() && (stat.mode & 0o777) === 0o600;
    });
    assert.equal((await lstat(configuration.socketPath)).mode & 0o777, 0o600);
    socket = connect(configuration.socketPath);
    await once(socket, "connect");
    client = new RuntimeClient(
      ndJsonStream(Writable.toWeb(socket), Readable.toWeb(socket)),
    );
    await client.initialize();
    const session = await client.createSession();
    await client.submit({
      schemaVersion: 4,
      kind: "command",
      commandId: "bundled",
      sessionId: session.namespace.sessionId,
      expiresAtMs: Date.now() + 30000,
      input: { type: "prompt", policy: "queue_next", text: "hello" },
    });
    await until(
      () =>
        client.getSession(session.namespace.sessionId).commands.bundled
          ?.state === "terminal",
    );
    assert.equal(
      client.getSession(session.namespace.sessionId).commands.bundled.outcome,
      "completed",
    );
    assert.ok(
      Object.values(
        client.getSession(session.namespace.sessionId).messages,
      ).some((message) => message.text === "bundled runtime"),
    );
    assert.equal(requests.length, 1);
    groups = execFileSync("/bin/ps", ["-ax", "-o", "pid=,ppid=,pgid="], {
      encoding: "utf8",
    })
      .trim()
      .split("\n")
      .map((line) => line.trim().split(/\s+/).map(Number))
      .filter(([pid, parent, pgid]) => parent === child.pid && pid === pgid)
      .map(([pid]) => pid);
    assert.equal(groups.length, 1);
    assert.equal(groupEmpty(groups[0]), false);
    // Keep the client attached: the CLI must close both socket and live worker.
    child.kill(signal);
    let timer;
    const status = await Promise.race([
      exited,
      new Promise((_, reject) => {
        timer = setTimeout(
          () => reject(new Error("CLI shutdown deadline")),
          5000,
        );
      }),
    ]).finally(() => clearTimeout(timer));
    assert.deepEqual(status, [0, null], stderr);
    await until(() => groups.every(groupEmpty));
    database = new DatabaseSync(configuration.databasePath, { readOnly: true });
    const record = JSON.parse(
      database.prepare("SELECT json FROM commands WHERE id='bundled'").get()
        .json,
    );
    assert.equal(record.state, "terminal");
    assert.equal(
      database.prepare("SELECT count(*) AS n FROM worker_launches").get().n,
      0,
    );
    await assert.rejects(
      lstat(configuration.socketPath),
      (error) => error.code === "ENOENT",
    );
    console.log(
      `Bundled CLI ${signal}: private socket, fixed model/real SDK, durable terminal, worker group and socket cleanup passed`,
    );
  } finally {
    await client?.close();
    socket?.destroy();
    database?.close();
    if (child && child.exitCode === null && child.signalCode === null) {
      child.kill("SIGKILL");
      await once(child, "exit");
    }
    // Only current test-launched groups are eligible for emergency cleanup.
    for (const pgid of groups)
      if (!groupEmpty(pgid)) {
        try {
          process.kill(-pgid, "SIGKILL");
        } catch {}
      }
    server.closeAllConnections();
    await new Promise((resolve) => server.close(resolve));
    await rm(directory, { recursive: true, force: true });
  }
}
