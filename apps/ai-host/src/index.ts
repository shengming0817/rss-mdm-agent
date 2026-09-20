import type { Duplex } from "node:stream";
import { Readable, Writable } from "node:stream";
import { lstat, mkdir } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { createHost } from "@rss-mdm-agent/ai-host";
import { openSqliteStore } from "@rss-mdm-agent/ai-store-sqlite";
import { createAccessService, type Stream } from "@rss-mdm-agent/ai-access";
import { connectExecution } from "./execution.js";
import { readConfiguration } from "./configuration.js";
import {
  boundedJson,
  decode,
  type Caller,
  type UserContext,
  type Connection,
} from "@rss-mdm-agent/ai-contract";
import { defaultLimits, fail } from "@rss-mdm-agent/ai-contract/transitions";
import { localResolver } from "./resolver.js";
import { ConnectionSecrets } from "./secrets.js";
import { NativeControl } from "./native.js";
export type { LocalConfiguration } from "./configuration.js";
/** One native-owned process, private control descriptor and fixed-context logical UI channels. */
export async function startLocalApp(
  configurationPath: string,
  parent = {
    input: process.stdin as Readable,
    output: process.stdout as Writable,
  },
  controlSocket?: Duplex,
) {
  const local = await readConfiguration(resolve(configurationPath));
  if (process.platform !== "darwin" || process.arch !== "arm64")
    throw new Error("runtime platform is not verified");
  const directory = dirname(local.databasePath);
  await mkdir(directory, { recursive: true, mode: 0o700 });
  const stat = await lstat(directory);
  if (
    !stat.isDirectory() ||
    stat.isSymbolicLink() ||
    (stat.mode & 0o077) !== 0 ||
    (process.getuid && stat.uid !== process.getuid())
  )
    throw new Error("runtime directory ownership");
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
  if (!opened.ok) throw new Error(opened.error.code);
  const store = opened.value;
  let activeUser: UserContext | undefined;
  const callerFor = (context: UserContext): Caller => ({
    tenantId: "test-users",
    principalId: context.user.userId,
    authorityId: "desktop-fixture",
  });
  const available = (caller: Caller) =>
    !!activeUser &&
    caller.principalId === activeUser.user.userId &&
    caller.tenantId === "test-users" &&
    caller.authorityId === "desktop-fixture";
  const execution = await connectExecution(
    parent.input,
    parent.output,
    async (request) => {
      if (!request.commandId) throw new Error("unbound origin");
      const command = await store.command(request.namespace, request.commandId),
        session = await store.session(request.namespace);
      if (!command.ok || !session.ok) throw new Error("unbound origin");
      const phase = session.value.stages.find(
        (stage) => stage.stageId === command.value.receipt.stageId,
      );
      if (!phase) throw new Error("unbound origin");
      return phase.binding;
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
    const bytes = await control.call("master_key", { create });
    if (
      !Array.isArray(bytes) ||
      bytes.length !== 32 ||
      bytes.some((n) => !Number.isInteger(n) || n < 0 || n > 255)
    )
      throw new Error("authentication_required");
    return Uint8Array.from(bytes);
  });
  const created = await createHost({
    delivery: execution?.router ?? null,
    store,
    launchFences: store,
    onDiagnostic: (diagnostic) =>
      process.stderr.write(`AI Host ${diagnostic.stage}: ${diagnostic.code}\n`),
    resolve: localResolver(local, store, secrets),
    callerAvailable: available,
    persistConnection: async (caller, connection, expected, secret, budget) => {
      const encrypted =
        connection.status !== "deleted" &&
        connection.source.type === "custom_api"
          ? await secrets.seal(
              caller,
              connection,
              await secrets.read(caller, connection, secret, expected ?? 0),
            )
          : undefined;
      if (!available(caller) || budget.signal.aborted)
        return fail("unavailable");
      return store.saveConnection(caller, connection, expected, encrypted);
    },
  });
  if (!created.ok) {
    await execution?.close();
    await store.close({
      timeoutMs: 1000,
      signal: new AbortController().signal,
    });
    throw new Error(created.error.code);
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
    async (method, data) => {
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
          if (activeUser && data.generation !== activeUser.generation)
            throw new Error("user changed");
          for (const id of views.keys()) detach(id);
          if (activeUser) {
            const fenced = await host.suspendCaller(callerFor(activeUser), {
              timeoutMs: 10000,
              signal: AbortSignal.timeout(10000),
            });
            if (!fenced.ok) throw new Error("user fence unavailable");
          }
          activeUser = undefined;
          return true;
        });
      if (method === "detach") {
        detach(data.channel);
        return true;
      }
      if (method === "save_connection") {
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
    (id, message) => {
      const view = views.get(id);
      if (!view) return;
      if (
        view.generation !== activeUser?.generation ||
        (view.input.desiredSize ?? 0) <= 0 ||
        Buffer.byteLength(JSON.stringify(message)) > 262144
      ) {
        detach(id);
        return;
      }
      view.input.enqueue(message);
    },
    controlSocket,
  );
  let closing: Promise<void> | undefined;
  const close = () =>
    (closing ??= (async () => {
      for (const id of views.keys()) detach(id);
      await service.close();
      await execution?.close();
      const result = await host.close({
        timeoutMs: 30000,
        signal: new AbortController().signal,
      });
      control.close();
      if (!result.ok) throw new Error(result.error.code);
    })());
  void control.stopped.then(close).catch(() => {
    process.exitCode = 1;
  });
  if (execution)
    void execution.stopped.then(close).catch(() => {
      process.exitCode = 1;
    });
  return { host, close };
}
