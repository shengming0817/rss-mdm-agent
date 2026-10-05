import { runCommand } from "./ci-plan.mjs";
import { withPreparation } from "./preparation.mjs";
import { privateRuntimeNode } from "./ai-host-artifacts.mjs";
import { sourceSummary } from "./connection-source-results.mjs";
import {
  mkdtemp,
  realpath,
  stat,
  rm,
  mkdir,
  writeFile,
  readFile,
} from "node:fs/promises";
import { tmpdir, homedir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
/** Explicit manual acceptance: passes existing local directories to the official tools and sends one probe per available source.
 * Emits only source type and closed outcome. Never emits account identities, paths or credentials. */
const repository = fileURLToPath(new URL("../", import.meta.url));
const preflight = process.argv.includes("--preflight");
const output = join(
  repository,
  ".local-ci-runs",
  preflight ? "connection-sources-preflight.json" : "connection-sources.json",
);
const receiptBase = {
  command: "pnpm check:connection-sources" + (preflight ? " --preflight" : ""),
  startedAt: new Date().toISOString(),
  runtime: {
    node: process.versions.node,
    platform: process.platform,
    arch: process.arch,
  },
  mode: preflight
    ? "native-private-storage/sqlite-preflight"
    : "official-config/isolated-provider/minimal-real-model-probe",
  notCovered: [
    "custom_api_covered_separately_by_check_native_credentials",
    "windows",
    "enterprise_identity",
  ],
};
let cancelled;
const interrupt = () => {
  cancelled = "SIGINT";
};
const terminate = () => {
  cancelled = "SIGTERM";
};
process.on("SIGINT", interrupt);
process.on("SIGTERM", terminate);
try {
  await mkdir(join(repository, ".local-ci-runs"), { recursive: true });
  await rm(output, { force: true });
  if (cancelled) throw Error("connection check cancelled before preparation");
  process.exitCode = await withPreparation(
    [
      "node:@rss-mdm-agent/ai-host-app",
      "node:@rss-mdm-agent/ai-client",
      "worker",
    ],
    async (env, signal, nested, assertReady) => {
      const { connectionPersistence } = await import(
        "../apps/ai-host/dist/secrets.js"
      );
      const { createHost } = await import("../packages/ai-host/dist/index.js");
      const { openSqliteStore } = await import(
        "../packages/ai-store-sqlite/dist/index.js"
      );
      const { localResolver } = await import(
        "../apps/ai-host/dist/resolver.js"
      );
      const fixedNode = await realpath(privateRuntimeNode(repository)).catch(
        () => {
          throw Error(
            "prepare the fixed AI runtime with pnpm check:connection-sources",
          );
        },
      );
      if ((await realpath(process.execPath)) !== fixedNode) {
        const result = await runCommand(
          fixedNode,
          [fileURLToPath(import.meta.url), ...process.argv.slice(2)],
          {
            cwd: repository,
            env,
            signal,
            stdio: "inherit",
            ...(nested ? { detached: false } : {}),
          },
        );
        return result.status ?? 1;
      }
      let sqliteReady = false;

      const budget = () => ({
        timeoutMs: 60000,
        signal: AbortSignal.any([signal, AbortSignal.timeout(60000)]),
      });
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
        const opened = await openSqliteStore({
          path: local.databasePath,
          mode: "create",
        });
        if (!opened.ok) throw Error(opened.error.code);
        const store = opened.value;
        if (preflight) {
          const closed = await store.close(budget());
          if (!closed.ok) throw Error(closed.error.code);
          sqliteReady = true;
        } else {
          const created = await createHost({
            credentialPersistence: connectionPersistence(
              store,
              () => true,
              async () => false,
            ),
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
                results.push({
                  provider,
                  source: type,
                  result: "source_absent",
                });
                continue;
              }
              const saved = await host.saveConnection(
                caller,
                {
                  connectionId: id,
                  name: id,
                  provider,
                  profile: "conversation",
                  source: { type, directory: directory ?? root },
                },
                null,
                budget(),
              );
              const tested = saved.ok
                ? await host.testConnection(
                    caller,
                    id,
                    saved.value.configRevision,
                    budget(),
                  )
                : saved;
              const row = {
                provider,
                source: type,
                result: !tested.ok
                  ? tested.error.code
                  : tested.value.lastTest?.outcome === "passed"
                    ? "model_probe_completed"
                    : (tested.value.lastTest?.failure.code ?? "unavailable"),
              };
              results.push(row);
              process.stdout.write(JSON.stringify(row) + "\n");
            }
        }
      } finally {
        if (host) await host.close(budget());
        await rm(root, { recursive: true, force: true });

        await new Promise(setImmediate);
        assertReady();
        await mkdir(join(repository, ".local-ci-runs"), { recursive: true });
        await writeFile(
          output,
          JSON.stringify(
            {
              ...receiptBase,
              finishedAt: new Date().toISOString(),
              ...(preflight
                ? {
                    status: sqliteReady ? "passed" : "failed",
                    sqliteReady,
                    probesRun: false,
                  }
                : sourceSummary(results)),
            },
            null,
            2,
          ),
        );
      }
      const failed = preflight
        ? !sqliteReady
        : sourceSummary(results).status === "failed";

      if (preflight)
        process.stdout.write(
          JSON.stringify({ sqliteReady, probesRun: false }) + "\n",
        );

      return failed ? 1 : 0;
    },
  );
} catch (error) {
  const previous = await readFile(output, "utf8")
    .then(JSON.parse)
    .catch(() => receiptBase);
  await mkdir(join(repository, ".local-ci-runs"), { recursive: true });
  await writeFile(
    output,
    JSON.stringify(
      {
        ...previous,
        finishedAt: new Date().toISOString(),
        status: cancelled ? "cancelled" : "failed",
        ...(cancelled ? { signal: cancelled } : {}),
        failure: error.message,
      },
      null,
      2,
    ),
  );
  console.error(error.message);
  process.exitCode = 1;
} finally {
  process.removeListener("SIGINT", interrupt);
  process.removeListener("SIGTERM", terminate);
}
