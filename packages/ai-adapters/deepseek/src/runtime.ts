import { fork, type ChildProcess } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { Budget } from "@rss-mdm-agent/ai-contract";
import type { DeepSeekDiagnostic } from "./configuration.js";
import {
  NativeFault,
  decodeFrame,
  decodeRequest,
  decodeResponse,
  type OperationRequest,
  type OperationResponse,
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
      resolve: (v: unknown) => void;
      reject: (reason?: DeepSeekDiagnostic["reason"]) => void;
    }
  >();
  let child: ChildProcess | undefined,
    next = 0,
    ended = false,
    stopping = false,
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
        if (!stopping) lost();
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
    child.on("message", (raw: unknown) => {
      try {
        const m = decodeFrame(copy(raw));
        if (m.type === "event") listener(m.event);
        else if (m.type === "reply") {
          const p = pending.get(m.id);
          pending.delete(m.id);
          if (m.ok) p?.resolve(m.value);
          else p?.reject(m.reason);
        } else throw new NativeFault("protocol_failure");
      } catch {
        exitReason = "protocol_failure";
        child!.kill("SIGKILL");
      }
    });
    child.once("close", finalize);
    child.on("error", () => {
      if (!child!.pid) {
        exitReason = "spawn_failed";
        for (const p of pending.values()) p.reject(exitReason);
        pending.clear();
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
      if (ended && !stopping) lost();
    },
    stop() {
      stopping = true;
      if (ended) cleanup();
      else child?.kill("SIGKILL");
    },
    async call<K extends Operation>(
      operation: K,
      value: OperationRequest[K],
      budget: Budget,
    ): Promise<OperationResponse[K]> {
      if (ended || !child || !liveBudget(budget) || pending.size >= 64)
        throw new NativeFault(ended ? exitReason : "native_failure");
      const id = ++next;
      const result = new Promise<OperationResponse[K]>((resolve, reject) => {
        pending.set(id, {
          resolve: (raw) => {
            try {
              resolve(decodeResponse(operation, raw));
            } catch {
              reject(new NativeFault("protocol_failure"));
            }
          },
          reject: (reason = "native_failure") =>
            reject(new NativeFault(reason)),
        });
        child!.send(decodeRequest(copy({ id, operation, value })), (error) => {
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
