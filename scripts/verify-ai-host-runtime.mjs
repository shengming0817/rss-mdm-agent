import { scopeAbsent } from "../packages/ai-host/dist/process.js";
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { PrivateLink } from "../packages/ai-host/dist/private-link.js";
import {
  configuration as fixtureConfiguration,
  nativePeer,
  clientAt,
  executionGeneration,
} from "../tests/ai-provider-conformance/support.mjs";
import { executionServer } from "../tests/ai-host/rust-execution.mjs";
import assert from "node:assert/strict";
import { spawn, execFileSync } from "node:child_process";
import { once } from "node:events";
import { mkdtemp, mkdir, writeFile, rm, realpath } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { DatabaseSync } from "node:sqlite";
import { createModelServer } from "../tests/ai-adapters/claude/model-fixture.mjs";

const runtimeRoot = resolve(process.argv[2]);
const executable = join(
  runtimeRoot,
  "bin/node" + (process.platform === "win32" ? ".exe" : ""),
);
const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
async function until(check) {
  const deadline = Date.now() + 15000;
  while (Date.now() < deadline) {
    if (await check()) return;
    await pause(20);
  }
  assert.fail("runtime lifecycle deadline");
}
const workerRuntime = {
  launcher: join(
    runtimeRoot,
    "bin/rss-ai-worker-launcher" + (process.platform === "win32" ? ".exe" : ""),
  ),
  manifestDigest: createHash("sha256")
    .update(readFileSync(join(runtimeRoot, "worker-manifest.json")))
    .digest("hex"),
};
for (const signal of ["SIGTERM", "SIGINT"]) {
  const directory = await realpath(
    await mkdtemp(join(tmpdir(), "rss bundled host-")),
  );
  const requests = [],
    server = createModelServer(
      [[{ type: "text", text: "bundled runtime" }]],
      requests,
    );
  let child, peer, client, database, rust;
  let groups = [],
    stderr = "";
  try {
    await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
    const setup = await fixtureConfiguration(
      directory,
      "claude",
      `http://127.0.0.1:${server.address().port}`,
    );
    const configuration = setup.config,
      configurationPath = setup.path;
    rust = spawn(
      executionServer(),
      [
        join(directory, "execution.sqlite"),
        join(directory, "audit.json"),
        "ai-unknown",
      ],
      { stdio: ["pipe", "pipe", "inherit"] },
    );
    rust.stdin.on("error", () => {});
    child = spawn(
      executable,
      [
        join(
          runtimeRoot,
          "node_modules/@rss-mdm-agent/ai-host-app/dist/cli.js",
        ),
        configurationPath,
      ],
      {
        cwd: directory,
        stdio: ["pipe", "pipe", "pipe"],
        detached: true,
        env: {
          PATH:
            process.platform === "win32" ? process.env.PATH : "/usr/bin:/bin",
          HOME: directory,
          TMPDIR: tmpdir(),
          SystemRoot: process.env.SystemRoot,
          USERPROFILE: directory,
          TEMP: tmpdir(),
        },
      },
    );
    const link = new PrivateLink(child.stdout, child.stdin, "native");
    link.lane("execution").pipe(rust.stdin);
    rust.stdout.pipe(link.lane("execution"));
    child.stdin.on("error", () => {});
    child.stderr.on("data", (chunk) => {
      stderr = (stderr + chunk).slice(-16384);
    });
    const exited = once(child, "exit");
    peer = nativePeer(link.lane("native"));
    const view = await clientAt(peer, await executionGeneration(directory));
    client = view.client;
    const session = await client.createSession();
    await client.submit({
      schemaVersion: 5,
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
    if (process.platform === "win32") {
      groups = JSON.parse(
        execFileSync(
          join(
            process.env.SystemRoot,
            "System32/WindowsPowerShell/v1.0/powershell.exe",
          ),
          [
            "-NoProfile",
            "-NonInteractive",
            "-File",
            fileURLToPath(
              new URL("./worker-scopes-windows.ps1", import.meta.url),
            ),
            "-ParentPid",
            String(child.pid),
          ],
          { encoding: "utf8" },
        ),
      );
    } else {
      groups = execFileSync("/bin/ps", ["-ax", "-o", "pid=,ppid=,pgid="], {
        encoding: "utf8",
      })
        .trim()
        .split("\n")
        .map((line) => line.trim().split(/\s+/).map(Number))
        .filter(([pid, parent, pgid]) => parent === child.pid && pid === pgid)
        .map(([root]) => ({ kind: "processGroup", root }));
    }
    assert.equal(groups.length, 1);
    assert.equal(scopeAbsent(workerRuntime, groups[0]), false);
    // Keep the client attached: the CLI must close both private channel and live worker.
    if (process.platform === "win32") peer.control.close();
    else child.kill(signal);
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
    await until(() =>
      groups.every((scope) => scopeAbsent(workerRuntime, scope)),
    );
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
    await peer.control.stopped;
    console.log(
      `Bundled CLI ${signal}: inherited private channel, fixed model/real SDK, durable terminal, worker group and channel cleanup passed`,
    );
  } finally {
    await client?.close();
    peer?.control.close();
    database?.close();
    if (child && child.exitCode === null && child.signalCode === null) {
      child.kill("SIGKILL");
      await once(child, "exit");
    }
    if (groups.length)
      await until(() =>
        groups.every((scope) => scopeAbsent(workerRuntime, scope)),
      );
    if (rust && rust.exitCode === null && rust.signalCode === null) {
      const exited = once(rust, "exit");
      rust.kill("SIGTERM");
      await exited;
    }
    server.closeAllConnections();
    await new Promise((resolve) => server.close(resolve));
    await rm(directory, { recursive: true, force: true });
  }
}
