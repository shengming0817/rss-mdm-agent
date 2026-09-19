import { fork, type ChildProcess } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { Budget } from "@rss-mdm-agent/ai-contract";
import type { DeepSeekDiagnostic } from "./configuration.js";
import {
  NativeFault,
  diagnosticReasons,
  type NativeEvent,
  type NativeRuntime,
  type Operation,
} from "./protocol.js";
import { bounded, copy, deferred, liveBudget } from "./support.js";
/** One private fixed-operation IPC channel. The spawn seam is internal only. */
export function nativeRuntime(spawn: typeof fork = fork): NativeRuntime {
  const env: Record<string, string> = {};
  for (const key of ["PATH", "SystemRoot", "WINDIR", "TEMP", "TMP", "TMPDIR"])
    if (process.env[key]) env[key] = process.env[key]!;
  const home = mkdtempSync(join(tmpdir(), "rss-deepseek-"));
  env.DSH_HOME = home;
  const stopped = deferred<void>();
  const pending = new Map<
    number,
    {
      resolve: (v: any) => void;
      reject: (reason?: DeepSeekDiagnostic["reason"]) => void;
    }
  >();
  let child: ChildProcess | undefined,
    next = 0,
    ended = false,
    exitReason: DeepSeekDiagnostic["reason"] = "process_exit",
    listener: (event: NativeEvent) => void = () => {};
  const lost = () =>
    listener({
      type: "lost",
      diagnostic: { stage: "process", reason: exitReason },
    });
  function cleanup() {
    try {
      rmSync(home, { recursive: true, force: true });
      stopped.resolve();
    } catch {
      listener({
        type: "error",
        diagnostic: { stage: "close", reason: "cleanup_failed" },
      });
      // A later close retries; process exit alone does not prove cleanup.
    }
  }
  function finalize() {
    if (!ended) {
      ended = true;
      for (const p of pending.values()) p.reject(exitReason);
      pending.clear();
      try {
        lost();
      } finally {
        cleanup();
      }
    } else cleanup();
  }
  try {
    child = spawn(new URL("./worker.js", import.meta.url), [], {
      env,
      execArgv: [],
      stdio: ["ignore", "ignore", "ignore", "ipc"],
      serialization: "json",
    });
    child.on("message", (raw: any) => {
      try {
        const m = copy(raw);
        if (m.type === "event") listener(m.event);
        else if (m.type === "reply") {
          const p = pending.get(m.id);
          pending.delete(m.id);
          if (m.ok) p?.resolve(m.value);
          else
            p?.reject(
              diagnosticReasons.includes(m.reason)
                ? m.reason
                : "native_failure",
            );
        } else throw new NativeFault("protocol_failure");
      } catch {
        exitReason = "protocol_failure";
        child!.kill("SIGKILL");
      }
    });
    child.once("exit", finalize);
    child.once("close", finalize);
    child.on("error", () => {
      if (!child!.pid) {
        exitReason = "spawn_failed";
        finalize();
      } else {
        exitReason = "protocol_failure";
        for (const p of pending.values()) p.reject(exitReason);
        pending.clear();
        child!.kill("SIGKILL");
      }
    });
  } catch {
    exitReason = "spawn_failed";
    finalize();
  }
  return {
    stopped: stopped.promise,
    onEvent(fn) {
      listener = fn;
      if (ended) lost();
    },
    stop() {
      if (ended) cleanup();
      else child?.kill("SIGKILL");
    },
    async call(operation: Operation, value: unknown, budget: Budget) {
      if (ended || !child || !liveBudget(budget) || pending.size >= 64)
        throw new NativeFault(ended ? exitReason : "native_failure");
      const id = ++next;
      const result = new Promise<any>((resolve, reject) => {
        pending.set(id, {
          resolve,
          reject: (reason = "native_failure") =>
            reject(new NativeFault(reason)),
        });
        child!.send(copy({ id, operation, value }), (error) => {
          if (error) {
            pending.get(id)?.reject("protocol_failure");
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
