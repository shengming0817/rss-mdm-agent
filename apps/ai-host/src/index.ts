import { privateDirectory } from "./private-file.js";
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { Readable, Writable } from "node:stream";
import { lstat } from "node:fs/promises";
import { dirname, resolve, join } from "node:path";
import { PrivateLink } from "@rss-mdm-agent/ai-host/private-link";
import { createHost } from "@rss-mdm-agent/ai-host";
import { openSqliteStore } from "@rss-mdm-agent/ai-store-sqlite";
import { createAccessService, type Stream } from "@rss-mdm-agent/ai-access";
import { connectExecution } from "./execution.js";
import { readConfiguration, ConfigurationError } from "./configuration.js";
import {
  boundedJson,
  decode,
  type Caller,
  type Budget,
  type Result,
  type UserContext,
  type Connection,
  type HostHealth,
} from "@rss-mdm-agent/ai-contract";
import { defaultLimits, fail } from "@rss-mdm-agent/ai-contract/transitions";
import { localResolver } from "./resolver.js";
import { connectionPersistence, ConnectionSecrets } from "./secrets.js";
import { NativeControl } from "./native.js";
export type { LocalConfiguration } from "./configuration.js";
const callerFor = (context: UserContext): Caller => ({
  tenantId: "test-users",
  principalId: context.user.userId,
  authorityId: "desktop-fixture",
});
/** Restart-safe user fence used by the private Native control handler. */
export async function suspendNativeCaller(
  host: {
    suspendCaller(caller: Caller, budget: Budget): Promise<Result<void>>;
  },
  previous: UserContext,
  active?: UserContext,
): Promise<void> {
  if (active && previous.generation !== active.generation)
    throw new Error("user changed");
  const fenced = await host.suspendCaller(callerFor(active ?? previous), {
    timeoutMs: 10000,
    signal: AbortSignal.timeout(10000),
  });
  if (!fenced.ok) throw new Error("user fence unavailable");
}
/** Stop every independent owner even when an earlier owner fails. */
export async function closeOwners(
  owners: readonly (() => Promise<unknown>)[],
): Promise<void> {
  const settled = await Promise.allSettled(owners.map((stop) => stop()));
  const failures = settled.flatMap((result) =>
    result.status === "rejected" ? [result.reason] : [],
  );
  if (failures.length) throw new AggregateError(failures, "close failed");
}
/** One native-owned process, private control descriptor and fixed-context logical UI channels. */
export async function startLocalApp(
  configurationPath: string,
  parent = {
    input: process.stdin as Readable,
    output: process.stdout as Writable,
  },
) {
  const local = await readConfiguration(resolve(configurationPath));
  if (
    !(
      (process.platform === "darwin" && process.arch === "arm64") ||
      (process.platform === "win32" && process.arch === "x64")
    )
  )
    throw new Error("runtime platform is not verified");
  const directory = dirname(local.databasePath);
  await privateDirectory(directory);
  const exists = await lstat(local.databasePath).then(
    () => true,
    (error) => {
      if (error.code === "ENOENT") return false;
      throw error;
    },
  );
  const opened = openSqliteStore({
    path: local.databasePath,
    mode: exists ? "open" : "create",
  });
  if (!opened.ok)
    throw new ConfigurationError(
      opened.error.code === "storage_corrupt"
        ? "storage_corrupt"
        : opened.error.code === "unsupported_version"
          ? "unsupported_version"
          : "startup_failed",
    );
  const store = opened.value;
  let activeUser: UserContext | undefined;
  const available = (caller: Caller) =>
    !!activeUser &&
    caller.principalId === activeUser.user.userId &&
    caller.tenantId === "test-users" &&
    caller.authorityId === "desktop-fixture";
  const link = new PrivateLink(parent.input, parent.output, "native");
  const executionLane = link.lane("execution");
  const execution = await connectExecution(
    executionLane,
    executionLane,
    async (request) => {
      const current = activeUser;
      if (!current || current.user.userId !== request.namespace.principalId)
        throw new Error("unbound origin");
      if (!request.commandId) throw new Error("unbound origin");
      const command = await store.command(request.namespace, request.commandId),
        session = await store.session(request.namespace);
      if (!command.ok || !session.ok) throw new Error("unbound origin");
      const phase = session.value.stages.find(
        (stage) => stage.stageId === command.value.receipt.stageId,
      );
      if (!phase) throw new Error("unbound origin");
      return { binding: phase.binding, userGeneration: current.generation };
    },
  ).catch(async (error) => {
    await store.close({
      timeoutMs: 1000,
      signal: new AbortController().signal,
    });
    throw error;
  });
  let control: NativeControl;
  const secrets = new ConnectionSecrets(store, async (create) => {
    const bytes = await control.call("masterKey", { create });
    if (
      !Array.isArray(bytes) ||
      bytes.length !== 32 ||
      bytes.some((n) => !Number.isInteger(n) || n < 0 || n > 255)
    )
      throw new Error("authentication_required");
    return Uint8Array.from(bytes);
  });
  const runtimeRoot = dirname(dirname(process.execPath));
  const created = await createHost({
    workerRuntime: {
      launcher: join(
        runtimeRoot,
        "bin",
        "rss-ai-worker-launcher" + (process.platform === "win32" ? ".exe" : ""),
      ),
      manifestDigest: createHash("sha256")
        .update(readFileSync(join(runtimeRoot, "worker-manifest.json")))
        .digest("hex"),
    },
    delivery: execution?.router ?? null,
    store,
    launchFences: store,
    onDiagnostic: (diagnostic) =>
      process.stderr.write(`AI Host ${diagnostic.stage}: ${diagnostic.code}\n`),
    resolve: localResolver(local, store, secrets),
    callerAvailable: available,
    persistConnection: connectionPersistence(store, secrets, available),
  });
  if (!created.ok) {
    await execution?.close();
    await store.close({
      timeoutMs: 1000,
      signal: new AbortController().signal,
    });
    throw new ConfigurationError(
      created.error.code === "storage_corrupt"
        ? "storage_corrupt"
        : "startup_failed",
    );
  }
  const host = created.value,
    service = createAccessService({ host, sessionOptions: {} });
  const views = new Map<
    string,
    { input: ReadableStreamDefaultController<any>; generation: string }
  >();
  const detach = (id: string) => {
    const view = views.get(id);
    if (view) {
      views.delete(id);
      try {
        view.input.close();
      } catch {}
    }
  };
  let switching: Promise<unknown> = Promise.resolve();
  const switchUser = (action: () => Promise<unknown>) => {
    const task = switching.catch(() => {}).then(action);
    switching = task;
    return task;
  };
  const context = (data: unknown): UserContext => {
    const record = decode(boundedJson(data, defaultLimits), defaultLimits);
    if (record.kind !== "userContext") throw new Error("invalid context");
    return record;
  };
  control = new NativeControl(
    async ({ method, data }) => {
      if (method === "health")
        return {
          schemaVersion: 5,
          kind: "hostHealth",
          ready: true,
          protocol: 3,
        } satisfies HostHealth;
      if (method === "attach")
        return switchUser(async () => {
          const next = context(data.context),
            id = data.channel;
          if (
            typeof id !== "string" ||
            id.length > 128 ||
            views.has(id) ||
            views.size >= 4
          )
            throw new Error("view limit");
          if (activeUser?.generation !== next.generation) {
            for (const id of views.keys()) detach(id);
            if (activeUser) {
              const fenced = await host.suspendCaller(callerFor(activeUser), {
                timeoutMs: 10000,
                signal: AbortSignal.timeout(10000),
              });
              if (!fenced.ok) throw new Error("user fence unavailable");
            }
            activeUser = next;
            host.activateCaller(callerFor(next));
          }
          const readable = new ReadableStream(
            {
              start(input) {
                views.set(id, { input, generation: next.generation });
              },
              cancel() {
                detach(id);
              },
            },
            { highWaterMark: 64 },
          );
          const writable = new WritableStream({
            write(message) {
              control.emit(id, message);
            },
            close() {
              detach(id);
            },
            abort() {
              detach(id);
            },
          });
          service.connect({ readable, writable } as Stream, callerFor(next));
          return true;
        });
      if (method === "suspend")
        return switchUser(async () => {
          const previous = context(data.context);
          for (const id of views.keys()) detach(id);
          await suspendNativeCaller(host, previous, activeUser);
          activeUser = undefined;
          return true;
        });
      if (method === "detach") {
        detach(data.channel);
        return true;
      }
      if (method === "saveConnection") {
        const current = activeUser;
        if (!current || current.generation !== data.generation)
          return fail("unavailable");
        const connection = decode(
          boundedJson(data.connection, defaultLimits),
          defaultLimits,
        );
        if (
          connection.kind !== "connection" ||
          !(data.expected === null || Number.isSafeInteger(data.expected)) ||
          (data.secret != null && typeof data.secret !== "string")
        )
          return fail("invalid_input");
        return host.saveConnection(
          callerFor(current),
          connection as Connection,
          data.expected,
          { timeoutMs: 90000, signal: AbortSignal.timeout(90000) },
          data.secret ?? undefined,
        );
      }
      throw new Error("unknown native method");
    },
    ({ channel, message }) => {
      const view = views.get(channel);
      if (!view) return;
      if (
        view.generation !== activeUser?.generation ||
        (view.input.desiredSize ?? 0) <= 0 ||
        Buffer.byteLength(JSON.stringify(message)) > 262144
      ) {
        detach(channel);
        return;
      }
      view.input.enqueue(message);
    },
    link.lane("native"),
  );
  let closing: Promise<void> | undefined;
  const close = () => {
    if (closing) return closing;
    const task = (async () => {
      for (const id of views.keys()) detach(id);
      await closeOwners([
        () => service.close(),
        () => execution?.close() ?? Promise.resolve(),
        async () => {
          const result = await host.close({
            timeoutMs: 30000,
            signal: new AbortController().signal,
          });
          if (!result.ok) throw new Error(result.error.code);
        },
        async () => control.close(),
      ]);
    })();
    closing = task;
    void task.catch(() => {
      if (closing === task) closing = undefined;
    });
    return task;
  };
  void control.stopped.then(close).catch(() => {
    process.exitCode = 1;
  });
  if (execution)
    void execution.stopped.then(close).catch(() => {
      process.exitCode = 1;
    });
  return { host, close };
}
