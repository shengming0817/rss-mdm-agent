import { fork } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { Budget } from "@rss-mdm-agent/ai-contract";
import type { NativeEvent, NativeRuntime, Operation } from "./protocol.js";
import { bounded, copy, deferred, liveBudget } from "./support.js";
/** One private fixed-operation IPC channel. No endpoint/module names cross this boundary. */
export function nativeRuntime(): NativeRuntime {
  const env: Record<string, string> = {};
  for (const key of ["PATH", "SystemRoot", "WINDIR", "TEMP", "TMP", "TMPDIR"])
    if (process.env[key]) env[key] = process.env[key]!;
  const home = mkdtempSync(join(tmpdir(), "rss-deepseek-"));
  env.DSH_HOME = home;
  const child = fork(new URL("./worker.js", import.meta.url), [], {
    env,
    execArgv: [],
    stdio: ["ignore", "ignore", "ignore", "ipc"],
    serialization: "json",
  });
  const stopped = deferred<void>();
  const pending = new Map<
    number,
    { resolve: (v: any) => void; reject: () => void }
  >();
  let next = 0,
    ended = false,
    listener: (event: NativeEvent) => void = () => {};
  child.on("message", (raw: any) => {
    try {
      const m = copy(raw);
      if (m.type === "event") listener(m.event);
      else if (m.type === "reply") {
        const p = pending.get(m.id);
        pending.delete(m.id);
        m.ok ? p?.resolve(m.value) : p?.reject();
      } else throw Error("invalid frame");
    } catch {
      child.kill("SIGKILL");
    }
  });
  child.once("exit", () => {
    ended = true;
    for (const p of pending.values()) p.reject();
    pending.clear();
    listener({ type: "lost" });
    cleanup();
  });
  function cleanup() {
    try {
      rmSync(home, { recursive: true, force: true });
      stopped.resolve();
    } catch {
      /* A later close retries cleanup; never acknowledge it early. */
    }
  }

  child.once("error", () => {
    for (const p of pending.values()) p.reject();
    pending.clear();
  });
  return {
    stopped: stopped.promise,
    onEvent(fn) {
      listener = fn;
    },
    stop() {
      if (!ended) child.kill("SIGKILL");
      else cleanup();
    },
    async call(operation: Operation, value: unknown, budget: Budget) {
      if (ended || !liveBudget(budget) || pending.size >= 64)
        throw Error("unavailable");
      const id = ++next;
      const result = new Promise<any>((resolve, reject) => {
        pending.set(id, {
          resolve,
          reject: () => reject(Error("native operation failed")),
        });
        child.send(copy({ id, operation, value }), (error) => {
          if (error) {
            pending.get(id)?.reject();
            pending.delete(id);
          }
        });
      });
      try {
        return await bounded(result, budget);
      } finally {
        pending.delete(id);
      }
    },
  };
}
