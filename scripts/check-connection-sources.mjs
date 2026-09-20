import { spawn, execFileSync } from "node:child_process";
import { once } from "node:events";
import { mkdtemp, realpath, readFile, stat, rm } from "node:fs/promises";
import { tmpdir, homedir } from "node:os";
import { join } from "node:path";
import { createHost } from "../packages/ai-host/dist/index.js";
import { openSqliteStore } from "../packages/ai-store-sqlite/dist/index.js";
import { localResolver } from "../apps/ai-host/dist/resolver.js";
/** Explicit manual acceptance: reads existing local sources and sends one probe per available source.
 * Emits only source type and closed outcome. Never emits account identities, paths or credentials. */
const budget = () => ({ timeoutMs: 60000, signal: AbortSignal.timeout(60000) });
const root = await realpath(
  await mkdtemp(join(tmpdir(), "rss-source-acceptance-")),
);
let host, broker;
const results = [];
try {
  const build = execFileSync(
    "cargo",
    [
      "build",
      "--locked",
      "--message-format=json",
      "-p",
      "rss-mdm-desktop",
      "--example",
      "connection-acceptance-broker",
    ],
    { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] },
  );
  const executable = build
    .trim()
    .split("\n")
    .map(JSON.parse)
    .find(
      (row) =>
        row.target?.name === "connection-acceptance-broker" && row.executable,
    )?.executable;
  if (!executable) throw Error("native_broker_unavailable");
  broker = spawn(executable, [root], { stdio: ["pipe", "ignore", "ignore"] });
  for (let i = 0; ; i++) {
    if (
      await stat(join(root, "credentials.sock")).then(
        (s) => s.isSocket(),
        () => false,
      )
    )
      break;
    if (i > 100) throw Error("native_broker_unavailable");
    await new Promise((r) => setTimeout(r, 20));
  }
  const registry = JSON.parse(await readFile(join(root, "users.json"), "utf8"));
  const caller = {
    tenantId: "test-users",
    principalId: registry.current.user.userId,
    authorityId: "desktop-fixture",
  };
  const local = {
    version: 1,
    databasePath: join(root, "ai.sqlite"),
    socketPath: join(root, "ai.sock"),
    credentialSocket: join(root, "credentials.sock"),
    usersPath: join(root, "users.json"),
    nativeDirectory: join(root, "native"),
    workingDirectory: root,
  };
  const opened = openSqliteStore({ path: local.databasePath, mode: "create" });
  if (!opened.ok) throw Error(opened.error.code);
  const store = opened.value;
  const created = await createHost({
    store,
    launchFences: store,
    delivery: null,
    operationTimeoutMs: 60000,
    resolve: localResolver(local, store),
  });
  if (!created.ok) throw Error(created.error.code);
  host = created.value;
  for (const provider of ["codex", "claude"])
    for (const type of ["existing_login", "existing_api"]) {
      const id = provider + "-" + type;
      const directory = await realpath(
        join(homedir(), provider === "codex" ? ".codex" : ".claude"),
      ).catch(() => undefined);
      if (!directory) {
        results.push({ provider, source: type, result: "source_absent" });
        continue;
      }
      const saved = await host.saveConnection(
        caller,
        {
          schemaVersion: 5,
          kind: "connection",
          connectionId: id,
          name: id,
          provider,
          configRevision: 1,
          credentialRevision: 1,
          accountRef: crypto.randomUUID(),
          credentialRef: crypto.randomUUID(),
          profile: "conversation",
          status: "unverified",
          source: { type, directory },
        },
        null,
        budget(),
      );
      const row = {
        provider,
        source: type,
        result: saved.ok ? "model_probe_completed" : saved.error.code,
      };
      results.push(row);
      process.stdout.write(JSON.stringify(row) + "\n");
    }
} finally {
  if (host) await host.close(budget());
  if (broker && broker.exitCode === null && broker.signalCode === null) {
    const exited = once(broker, "exit");
    broker.stdin.end("\n");
    await exited;
  }
  await rm(root, { recursive: true, force: true });
}
if (results.some((row) => row.result !== "model_probe_completed"))
  process.exitCode = 1;
