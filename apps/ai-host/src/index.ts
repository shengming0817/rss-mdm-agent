import { privateDirectory } from "@rss-mdm-agent/platform-private-storage";
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { Readable, Writable } from "node:stream";
import { lstat } from "node:fs/promises";
import { dirname, resolve, join } from "node:path";
import { PrivateLink } from "@rss-mdm-agent/ai-host/private-link";
import { processWorkerAdapter } from "@rss-mdm-agent/ai-host/process";
import { createHost } from "@rss-mdm-agent/ai-host";
import { openSqliteStore } from "@rss-mdm-agent/ai-store-sqlite";
import { createAccessService, type Stream } from "@rss-mdm-agent/ai-access";
import { connectExecution } from "./execution.js";
import { readConfiguration, ConfigurationError } from "./configuration.js";
import { boundedJson, decode } from "@rss-mdm-agent/ai-contract";
import type { Caller, Budget, Result } from "@rss-mdm-agent/ai-contract/ports";
import type {
  UserContext,
  Connection,
  HostHealth,
} from "@rss-mdm-agent/ai-contract/wire";
import { defaultLimits, fail } from "@rss-mdm-agent/ai-contract/transitions";
import { localResolver } from "./resolver.js";
import { connectionPersistence, ConnectionSecrets } from "./secrets.js";
import { NativeControl } from "./native.js";
export type { LocalConfiguration } from "./configuration.js";
export const callerFor = (context: UserContext): Caller => ({
  tenantId: context.identity.tenantId,
  principalId: context.identity.principalId,
  authorityId: context.identity.authorityId,
});
/** Exact native caller binding, including an enterprise deadline of zero. */
export function callerAvailable(
  context: UserContext | undefined,
  caller: Caller,
  now = Date.now(),
): boolean {
  if (
    !context ||
    (context.identity.mode === "user_device" &&
      now >= context.identity.expiresAtMs)
  )
    return false;
  const expected = callerFor(context);
  return (
    expected.tenantId === caller.tenantId &&
    expected.principalId === caller.principalId &&
    expected.authorityId === caller.authorityId
  );
}
/** Execution requires the native owner binding, for both device and user/device contexts. */
export function requireLocalExecution(
  context: UserContext | undefined,
  caller: Caller,
): void {
  if (!context || !context.executionBinding) throw new Error("unbound origin");
  const expected = callerFor(context);
  if (
    expected.tenantId !== caller.tenantId ||
    expected.principalId !== caller.principalId ||
    expected.authorityId !== caller.authorityId
  )
    throw new Error("unbound origin");
}
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
      (process.platform === "win32" && ["x64", "arm64"].includes(process.arch))
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
  const opened = await openSqliteStore({
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
  const available = (caller: Caller) => callerAvailable(activeUser, caller);
  const link = new PrivateLink(parent.input, parent.output, "native");
  const executionLane = link.lane("execution");
  const execution = await connectExecution(
    executionLane,
    executionLane,
    async (request) => {
      const current = activeUser;
      requireLocalExecution(current, request.namespace);
      if (!current || !available(request.namespace))
        throw new Error("unbound origin");
      if (request.catalog) {
        const expected = request.catalog;
        const connection = await store.connection(
          request.namespace,
          expected.config.id,
          Number(expected.config.revision),
        );
        if (
          !connection.ok ||
          connection.value.provider !== expected.provider ||
          connection.value.status === "deleted"
        )
          throw new Error("unbound origin");
        return {
          binding: expected,
          userGeneration: current.generation,
          contextBinding: current.executionBinding!,
        };
      }
      if (!request.commandId) {
        const session = await store.session(request.namespace);
        if (!session.ok) throw new Error("unbound origin");
        const phase = session.value.stages.find(
          (stage) => stage.stageId === session.value.currentStageId,
        );
        if (phase)
          return {
            binding: phase.binding,
            userGeneration: current.generation,
            contextBinding: current.executionBinding!,
          };
        if (!session.value.selectedConnectionId)
          throw new Error("unbound origin");
        const connection = await store.connection(
          request.namespace,
          session.value.selectedConnectionId,
        );
        if (!connection.ok) throw new Error("unbound origin");
        return {
          binding: {
            provider: connection.value.provider,
            config: {
              id: connection.value.connectionId,
              revision: String(connection.value.configRevision),
            },
          },
          userGeneration: current.generation,
          contextBinding: current.executionBinding!,
        };
      }
      const command = await store.command(request.namespace, request.commandId),
        session = await store.session(request.namespace);
      if (!command.ok || !session.ok) throw new Error("unbound origin");
      const phase = session.value.stages.find(
        (stage) => stage.stageId === command.value.receipt.stageId,
      );
      if (!phase) throw new Error("unbound origin");
      return {
        binding: phase.binding,
        userGeneration: current.generation,
        contextBinding: current.executionBinding!,
      };
    },
  ).catch(async (error) => {
    await store.close({
      timeoutMs: 1000,
      signal: new AbortController().signal,
    });
    throw error;
  });
  let control: NativeControl;
  const secrets = new ConnectionSecrets(
    store,
    (caller, connection, encrypted) =>
      control.call("openCredential", {
        caller,
        connection,
        encrypted:
          encrypted as import("@rss-mdm-agent/ai-contract/wire").EncryptedCredential,
      }),
  );
  const runtimeRoot = dirname(dirname(process.execPath));
  let permissionAccess: ReturnType<typeof createAccessService> | undefined;
  const created = await createHost({
    workers: processWorkerAdapter({
      launcher: join(
        runtimeRoot,
        "bin",
        "rss-ai-worker-launcher" + (process.platform === "win32" ? ".exe" : ""),
      ),
      manifestDigest: createHash("sha256")
        .update(readFileSync(join(runtimeRoot, "worker-manifest.json")))
        .digest("hex"),
    }),
    delivery: execution
      ? {
          router: execution.router,
          authorize: async (request, budget) => {
            if (
              request.proposal.name !== "execution_task_control" ||
              !permissionAccess ||
              !available(request.namespace) ||
              budget.signal.aborted
            )
              return {
                ok: false,
                error: { code: "unavailable", retry: "never" },
              };
            const answer = await permissionAccess.requestPermission(
              request.namespace,
              {
                sessionId: request.namespace.sessionId,
                toolCall: {
                  toolCallId: request.operationId,
                  title: "确认发送任务控制请求",
                  rawInput: request.proposal.arguments,
                },
                options: [
                  { optionId: "allow", kind: "allow_once", name: "发送请求" },
                  { optionId: "reject", kind: "reject_once", name: "返回" },
                ],
              },
              budget.signal,
            );
            if (
              budget.signal.aborted ||
              !available(request.namespace) ||
              answer.outcome.outcome !== "selected"
            )
              return {
                ok: false,
                error: { code: "unavailable", retry: "never" },
              };
            return {
              ok: true,
              value:
                answer.outcome.optionId === "allow" ? "allowed" : "rejected",
            };
          },
        }
      : null,
    store,
    launchFences: store,
    onDiagnostic: (diagnostic) =>
      process.stderr.write(`AI Host ${diagnostic.stage}: ${diagnostic.code}\n`),
    resolve: localResolver(local, store, secrets),
    callerAvailable: available,
    credentialPersistence: connectionPersistence(
      store,
      available,
      async (caller, previous, connection) =>
        (await control.call("matchCredential", {
          caller,
          previous,
          connection,
        })) === true,
    ),
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
  permissionAccess = service;
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
          schemaVersion: 1,
          kind: "hostHealth",
          ready: true,
          protocol: 1,
        } satisfies HostHealth;
      if (method === "refresh")
        return switchUser(async () => {
          const next = context(data.context);
          if (!activeUser) return true;
          if (
            activeUser.generation !== next.generation ||
            activeUser.executionBinding !== next.executionBinding ||
            JSON.stringify(callerFor(activeUser)) !==
              JSON.stringify(callerFor(next)) ||
            activeUser.identity.mode !== next.identity.mode
          )
            throw new Error("retired context");
          activeUser = next;
          return true;
        });
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
          if (
            JSON.stringify(callerFor(activeUser!)) !==
              JSON.stringify(callerFor(next)) ||
            activeUser!.executionBinding !== next.executionBinding
          )
            throw new Error("context changed");
          activeUser = next;
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
      if (method === "credentialContext") {
        if (!activeUser || activeUser.generation !== data.generation)
          throw new Error("unavailable");
        const existing = await store.hasSecrets();
        if (!existing.ok) throw new Error("unavailable");
        return { hasSecrets: existing.value };
      }
      if (method === "saveConnection") {
        const current = activeUser;
        if (!current || current.generation !== data.generation)
          return fail("unavailable");
        const caller = callerFor(current);
        const budget = { timeoutMs: 10000, signal: AbortSignal.timeout(10000) };
        return data.encrypted === null
          ? host.saveConnection(caller, data.connection, data.expected, budget)
          : host.saveNativeConnection(
              caller,
              data.connection,
              data.expected,
              budget,
              Uint8Array.from(data.encrypted),
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
