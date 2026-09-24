import { sourceSummary } from "./connection-source-results.mjs";
import {
  mkdtemp,
  realpath,
  stat,
  rm,
  mkdir,
  writeFile,
} from "node:fs/promises";
import { tmpdir, homedir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { createHost } from "../packages/ai-host/dist/index.js";
import { openSqliteStore } from "../packages/ai-store-sqlite/dist/index.js";
import { localResolver } from "../apps/ai-host/dist/resolver.js";
/** Explicit manual acceptance: passes existing local directories to the official tools and sends one probe per available source.
 * Emits only source type and closed outcome. Never emits account identities, paths or credentials. */
const repository = fileURLToPath(new URL("../", import.meta.url));

const startedAt = new Date().toISOString();
const budget = () => ({ timeoutMs: 60000, signal: AbortSignal.timeout(60000) });
const root = await realpath(
  await mkdtemp(join(tmpdir(), "rss-source-acceptance-")),
);
let host;
const results = [];
try {
  const caller = {
    tenantId: "test-users",
    principalId: "source-probe",
    authorityId: "desktop-fixture",
  };
  const local = {
    version: 1,
    databasePath: join(root, "ai.sqlite"),
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
    resolve: localResolver(local, store, {
      read: async () => {
        throw Error("custom key not part of existing-config probe");
      },
    }),
  });
  if (!created.ok) throw Error(created.error.code);
  host = created.value;
  for (const provider of ["codex", "claude"])
    for (const type of ["existing_config"]) {
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
          profile: "conversation",
          status: "unverified",
          source: { type, directory: directory ?? root },
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
  await rm(root, { recursive: true, force: true });

  const output = join(repository, ".local-ci-runs", "connection-sources.json");
  await mkdir(join(repository, ".local-ci-runs"), { recursive: true });
  await writeFile(
    output,
    JSON.stringify(
      {
        command: "node scripts/check-connection-sources.mjs",
        startedAt,
        finishedAt: new Date().toISOString(),
        runtime: {
          node: process.versions.node,
          platform: process.platform,
          arch: process.arch,
        },
        mode: "official-config/isolated-provider/minimal-real-model-probe",
        ...sourceSummary(results),
        notCovered: [
          "custom_api_covered_separately_by_check_native_credentials",
          "windows",
          "enterprise_identity",
        ],
      },
      null,
      2,
    ),
  );
}
if (sourceSummary(results).status === "failed") process.exitCode = 1;
