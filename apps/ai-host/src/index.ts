import { createServer, createConnection, type Socket } from "node:net";
import { Readable, Writable } from "node:stream";
import { chmod, lstat, mkdir, unlink } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { createHost } from "@rss-mdm-agent/ai-host";
import { openSqliteStore } from "@rss-mdm-agent/ai-store-sqlite";
import { createAccessService, ndJsonStream } from "@rss-mdm-agent/ai-access";
import { connectExecution } from "./execution.js";
import { readConfiguration } from "./configuration.js";
import { readPrivateFile } from "./private-file.js";
import {
  decode,
  type Caller,
  type UserContext,
} from "@rss-mdm-agent/ai-contract";
import { defaultLimits } from "@rss-mdm-agent/ai-contract/transitions";
import { localResolver } from "./resolver.js";
export type { LocalConfiguration } from "./configuration.js";
/** A private local ACP endpoint. Disconnecting a socket only detaches that client. */
export async function startLocalApp(
  configurationPath: string,
  parent = {
    input: process.stdin as Readable,
    output: process.stdout as Writable,
  },
) {
  const path = resolve(configurationPath),
    local = await readConfiguration(path);
  if (process.platform !== "darwin" || process.arch !== "arm64")
    throw new Error("runtime platform is not verified");
  for (const directory of new Set([
    dirname(local.databasePath),
    dirname(local.socketPath),
  ])) {
    await mkdir(directory, { recursive: true, mode: 0o700 });
    const stat = await lstat(directory);
    if (
      !stat.isDirectory() ||
      stat.isSymbolicLink() ||
      (stat.mode & 0o077) !== 0 ||
      (process.getuid && stat.uid !== process.getuid())
    )
      throw new Error("runtime directory ownership");
  }
  const existing = await lstat(local.databasePath).then(
    () => true,
    (error) => {
      if (error.code === "ENOENT") return false;
      throw error;
    },
  );
  const opened = openSqliteStore({
    path: local.databasePath,
    mode: existing ? "open" : "create",
  });
  if (!opened.ok) throw new Error(opened.error.code);
  const store = opened.value;
  const readUser = async (): Promise<UserContext | undefined> => {
    const record = decode(
      await readPrivateFile(local.usersPath, 65536),
      defaultLimits,
    );
    if (record.kind !== "testUserPage")
      throw new Error("invalid user registry");
    return record.current;
  };
  const callerFor = (context: UserContext): Caller => ({
    tenantId: "test-users",
    principalId: context.user.userId,
    authorityId: "desktop-fixture",
  });
  let activeUser = await readUser();
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
  const created = await createHost({
    delivery: execution?.router ?? null,
    store,
    launchFences: store,
    onDiagnostic: (diagnostic) =>
      process.stderr.write(`AI Host ${diagnostic.stage}: ${diagnostic.code}\n`),
    resolve: localResolver(local, store),
    callerAvailable: (caller) =>
      !!activeUser &&
      caller.principalId === activeUser.user.userId &&
      caller.tenantId === "test-users" &&
      caller.authorityId === "desktop-fixture",
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
    service = createAccessService({ host, sessionOptions: {} }),
    sockets = new Set<Socket>();
  let switching: Promise<unknown> = Promise.resolve();
  const server = createServer((socket) => {
    sockets.add(socket);
    socket.once("close", () => sockets.delete(socket));
    socket.on("error", () => socket.destroy());
    void (async () => {
      const request = await nativeHandshake(socket);
      const change = switching
        .catch(() => {})
        .then(async () => {
          const next = await readUser();
          if (!next || request.generation !== next.generation)
            throw new Error("user_changed");
          if (activeUser?.generation !== next.generation) {
            if (activeUser) {
              for (const previous of sockets)
                if (previous !== socket) previous.destroy();
              await host.suspendCaller(callerFor(activeUser), {
                timeoutMs: 10000,
                signal: AbortSignal.timeout(10000),
              });
            }
            activeUser = next;
            host.activateCaller(callerFor(next));
          }
          return callerFor(next);
        });
      switching = change;
      const caller = await change;
      if (request.type === "select_user") {
        socket.end(JSON.stringify({ ok: true }) + "\n");
        return;
      }
      const stream = ndJsonStream(
        Writable.toWeb(socket) as WritableStream<Uint8Array>,
        Readable.toWeb(socket) as ReadableStream<Uint8Array>,
      );
      service.connect(stream, caller);
      socket.resume();
    })().catch(() => socket.destroy());
  });
  let closed = false,
    ownsSocket = false;
  let socketIdentity: { dev: number; ino: number } | undefined;
  let closing: Promise<void> | undefined;
  const close = (): Promise<void> => {
    if (closed) return Promise.resolve();
    if (closing) return closing;
    closing = finishClose().finally(() => {
      closing = undefined;
    });
    return closing;
  };
  const finishClose = async () => {
    if (closed) return;
    const stopped = new Promise<void>((resolve) =>
      server.close(() => resolve()),
    );
    for (const socket of sockets) socket.destroy();
    await service.close();
    await execution?.close();
    await stopped;
    const result = await host.close({
      timeoutMs: 30000,
      signal: new AbortController().signal,
    });
    if (!result.ok) throw new Error(result.error.code);
    closed = true;
    if (ownsSocket && socketIdentity) {
      const remaining = await lstat(local.socketPath).catch((error) => {
        if (error.code === "ENOENT") return undefined;
        throw error;
      });
      if (
        remaining?.dev === socketIdentity.dev &&
        remaining.ino === socketIdentity.ino
      )
        await unlink(local.socketPath);
    }
  };
  try {
    // A configured path never authorizes deleting an ordinary file or another listener.
    const prior = await lstat(local.socketPath).catch((error) => {
      if (error.code === "ENOENT") return undefined;
      throw error;
    });
    if (prior) {
      if (!prior.isSocket()) throw new Error("socket path is not a socket");
      await new Promise<void>((resolve, reject) => {
        const probe = createConnection(local.socketPath);
        probe.setTimeout(300, () => {
          probe.destroy();
          reject(new Error("socket ownership unknown"));
        });
        probe.once("connect", () => {
          probe.destroy();
          reject(new Error("socket already in use"));
        });
        probe.once("error", (error) => {
          probe.destroy();
          if ((error as NodeJS.ErrnoException).code === "ECONNREFUSED")
            resolve();
          else reject(error);
        });
      });
      await unlink(local.socketPath);
    }
    await new Promise<void>((resolve, reject) => {
      server.once("error", reject);
      server.listen(local.socketPath, () => {
        ownsSocket = true;
        server.off("error", reject);
        resolve();
      });
    });
    socketIdentity = await lstat(local.socketPath);
    await chmod(local.socketPath, 0o600);
    if (execution)
      void execution.stopped
        .then(() => close())
        .catch(() => {
          process.stderr.write(
            "AI Host parent connection closed; cleanup incomplete\n",
          );
          process.exitCode = 1;
        });
    return { host, close };
  } catch (error) {
    await close();
    throw error;
  }
}

/** Consumed before ACP is connected; the WebView never supplies this native preface. */
function nativeHandshake(
  socket: Socket,
): Promise<{ type: "attach" | "select_user"; generation: string }> {
  return new Promise((resolve, reject) => {
    let bytes = Buffer.alloc(0);
    const timer = setTimeout(
      () => finish(new Error("native ingress timeout")),
      10000,
    );
    const finish = (
      error?: Error,
      value?: { type: "attach" | "select_user"; generation: string },
    ) => {
      clearTimeout(timer);
      socket.off("data", data).off("error", failure).off("end", failure);
      error ? reject(error) : resolve(value!);
    };
    const failure = () => finish(new Error("native ingress unavailable"));
    const data = (chunk: Buffer) => {
      bytes = Buffer.concat([bytes, chunk]);
      if (bytes.length > 262144)
        return finish(new Error("native ingress limit"));
      const end = bytes.indexOf(10);
      if (end < 0) return;
      socket.pause();
      try {
        const value = JSON.parse(bytes.subarray(0, end).toString("utf8"));
        if (
          Object.keys(value).length !== 2 ||
          !["attach", "select_user"].includes(value.type) ||
          typeof value.generation !== "string"
        )
          throw new Error();
        if (bytes.length > end + 1) socket.unshift(bytes.subarray(end + 1));
        finish(undefined, value);
      } catch {
        finish(new Error("native ingress rejected"));
      }
    };
    socket.on("data", data).once("error", failure).once("end", failure);
  });
}
