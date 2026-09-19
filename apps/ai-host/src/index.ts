import { createServer, createConnection, type Socket } from "node:net";
import { Readable, Writable } from "node:stream";
import { chmod, lstat, mkdir, unlink } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { createHost } from "@rss-mdm-agent/ai-host";
import { openSqliteStore } from "@rss-mdm-agent/ai-store-sqlite";
import { createAccessService, ndJsonStream } from "@rss-mdm-agent/ai-access";
import { readConfiguration } from "./configuration.js";
export type { LocalConfiguration } from "./configuration.js";
/** A private local ACP endpoint. Disconnecting a socket only detaches that client. */
export async function startLocalApp(configurationPath: string) {
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
  const artifact = new URL("./claude-provider.js", import.meta.url);
  artifact.searchParams.set("configuration", path);
  const created = await createHost({
    store,
    onDiagnostic: (diagnostic) =>
      process.stderr.write(`AI Host ${diagnostic.stage}: ${diagnostic.code}\n`),
    resolve: async (caller, options, namespace) => {
      if (
        caller.tenantId !== local.caller.tenantId ||
        caller.principalId !== local.caller.principalId ||
        caller.authorityId !== local.caller.authorityId ||
        options.provider !== local.session.provider ||
        options.config.id !== local.session.config.id ||
        options.config.revision !== local.session.config.revision ||
        options.accountRef !== local.session.accountRef ||
        options.profile !== local.session.profile
      )
        throw new Error("local scope mismatch");
      return {
        configuration: {
          namespace,
          provider: options.provider,
          config: options.config,
          accountRef: options.accountRef,
          workingDirectory: local.workingDirectory,
          permissions: "tools_disabled",
        },
        artifact: artifact.href,
      };
    },
  });
  if (!created.ok) {
    await store.close({
      timeoutMs: 1000,
      signal: new AbortController().signal,
    });
    throw new Error(created.error.code);
  }
  const host = created.value,
    service = createAccessService({ host, sessionOptions: local.session }),
    sockets = new Set<Socket>();
  const server = createServer((socket) => {
    sockets.add(socket);
    socket.once("close", () => sockets.delete(socket));
    socket.on("error", () => socket.destroy());
    const stream = ndJsonStream(
      Writable.toWeb(socket) as WritableStream<Uint8Array>,
      Readable.toWeb(socket) as ReadableStream<Uint8Array>,
    );
    service.connect(stream, local.caller);
  });
  let closed = false,
    ownsSocket = false;
  let socketIdentity: { dev: number; ino: number } | undefined;
  const close = async () => {
    if (closed) return;
    const stopped = new Promise<void>((resolve) =>
      server.close(() => resolve()),
    );
    for (const socket of sockets) socket.destroy();
    await service.close();
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
    return { host, close };
  } catch (error) {
    await close();
    throw error;
  }
}
