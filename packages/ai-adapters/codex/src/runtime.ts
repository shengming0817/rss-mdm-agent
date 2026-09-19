import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { readFileSync } from "node:fs";
import { StringDecoder } from "node:string_decoder";
import {
  boundedJson,
  type Budget,
  type Result,
} from "@rss-mdm-agent/ai-contract";
import {
  bounded,
  live,
  limits,
  NativeNotSubmittedError,
  ok,
  fail,
} from "./support.js";

export const CODEX_VERSION = "0.155.0";
export { NativeNotSubmittedError } from "./support.js";
export function classifyNativeRejection(
  method: string,
  error: unknown,
): NativeNotSubmittedError | undefined {
  if (
    CODEX_VERSION !== "0.155.0" ||
    method !== "turn/steer" ||
    !error ||
    typeof error !== "object" ||
    Array.isArray(error) ||
    JSON.stringify(Object.keys(error).sort()) !==
      JSON.stringify(["code", "message"]) ||
    (error as { code?: unknown }).code !== -32600 ||
    (error as { message?: unknown }).message !== "no active turn to steer"
  )
    return undefined;
  return new NativeNotSubmittedError();
}
export interface LaunchSpec {
  cwd: string;
  env: Record<string, string>;
  args: string[];
}
export interface NativeMessage {
  method: string;
  params: unknown;
  id?: string | number;
}
export interface RpcConnection {
  request(method: string, params: unknown, budget: Budget): Promise<unknown>;
  notify(method: string, params?: unknown): void;
  reply(id: string | number, result: unknown): void;
  reject(id: string | number): void;
  listen(handler: (message: NativeMessage) => void): void;
  readonly stopped: Promise<void>;
  close(budget: Budget): Promise<Result<{ processStopped: boolean }>>;
}
export type RuntimeFactory = (spec: LaunchSpec) => RpcConnection;
/** No PATH discovery or wrapper process: cleanup owns the actual pinned native process. */
export function codexBinary(): string {
  const require = createRequire(import.meta.url);
  const manifestPath = require.resolve("@openai/codex/package.json");
  if (JSON.parse(readFileSync(manifestPath, "utf8")).version !== CODEX_VERSION)
    throw new Error("Codex runtime version mismatch");
  const arch = ({ x64: "x86_64", arm64: "aarch64" } as Record<string, string>)[
    process.arch
  ];
  const target = (
    {
      darwin: "apple-darwin",
      linux: "unknown-linux-musl",
      win32: "pc-windows-msvc",
    } as Record<string, string>
  )[process.platform];
  if (!arch || !target) throw new Error("unsupported native platform");
  const nativeRequire = createRequire(manifestPath);
  const platformPackage = nativeRequire.resolve(
    `@openai/codex-${process.platform}-${process.arch}/package.json`,
  );
  if (
    JSON.parse(readFileSync(platformPackage, "utf8")).version !==
    `${CODEX_VERSION}-${process.platform}-${process.arch}`
  )
    throw new Error("Codex platform package mismatch");
  return join(
    dirname(platformPackage),
    "vendor",
    `${arch}-${target}`,
    "bin",
    process.platform === "win32" ? "codex.exe" : "codex",
  );
}

class StdioConnection implements RpcConnection {
  readonly stopped: Promise<void>;
  private child: ChildProcessWithoutNullStreams;
  private pending = new Map<
    number,
    {
      method: string;
      resolve(value: unknown): void;
      reject(error: Error): void;
    }
  >();
  private handler?: (message: NativeMessage) => void;
  private buffered: NativeMessage[] = [];
  private sequence = 0;
  private ended = false;
  private closing = false;
  constructor(spec: LaunchSpec) {
    this.child = spawn(codexBinary(), spec.args, {
      cwd: spec.cwd,
      env: spec.env,
      stdio: ["pipe", "pipe", "pipe"],
      windowsHide: true,
      detached: process.platform !== "win32",
    });
    this.child.stderr.resume(); // Native diagnostics may include prompts or credentials.
    this.stopped = new Promise<void>((resolve) => {
      const stopped = () => {
        if (this.ended) return;
        this.ended = true;
        for (const request of this.pending.values())
          request.reject(new Error("native process stopped"));
        this.pending.clear();
        resolve();
      };
      this.child.once("close", stopped);
      this.child.once("error", () => {
        if (!this.child.pid) stopped();
        else this.kill("SIGKILL");
      });
    });
    const decoder = new StringDecoder("utf8");
    let buffer = "";
    this.child.stdout.on("data", (chunk: Buffer) => {
      try {
        buffer += decoder.write(chunk);
        let newline;
        while ((newline = buffer.indexOf("\n")) >= 0) {
          const line = buffer.slice(0, newline);
          buffer = buffer.slice(newline + 1);
          if (Buffer.byteLength(line) > limits.maxBytes)
            throw new Error("frame limit");
          if (line.trim()) this.receive(JSON.parse(line));
        }
        if (Buffer.byteLength(buffer) > limits.maxBytes)
          throw new Error("frame limit");
      } catch {
        this.kill("SIGKILL");
      }
    });
    this.child.stdin.on("error", () => this.kill("SIGKILL"));
  }
  private receive(value: any): void {
    boundedJson(value, limits);
    if (!value || typeof value !== "object" || Array.isArray(value))
      throw new Error("invalid frame");
    if (typeof value.method === "string") {
      if (
        value.id !== undefined &&
        typeof value.id !== "string" &&
        typeof value.id !== "number"
      )
        throw new Error("invalid id");
      const message = {
        method: value.method,
        params: value.params,
        ...(value.id !== undefined ? { id: value.id } : {}),
      };
      if (this.handler) this.handler(message);
      else {
        if (this.buffered.length >= 64)
          throw new Error("pre-initialization message limit");
        this.buffered.push(message);
      }
    } else {
      if (
        typeof value.id !== "number" ||
        "result" in value === "error" in value
      )
        throw new Error("invalid response");
      const waiting = this.pending.get(value.id);
      if (!waiting) return; // Timed-out RPCs are reconciled through native history.
      this.pending.delete(value.id);
      if ("error" in value) {
        const definitive =
          JSON.stringify(Object.keys(value).sort()) ===
          JSON.stringify(["error", "id"])
            ? classifyNativeRejection(waiting.method, value.error)
            : undefined;
        waiting.reject(definitive ?? new Error("native RPC rejected"));
      } else waiting.resolve(value.result);
    }
  }
  private write(message: unknown): void {
    if (this.ended || this.closing || this.child.stdin.destroyed)
      throw new Error("closed native process");
    if (this.child.stdin.writableLength > limits.maxBytes)
      throw new Error("native write limit");
    this.child.stdin.write(boundedJson(message, limits) + "\n");
  }
  async request(
    method: string,
    params: unknown,
    budget: Budget,
  ): Promise<unknown> {
    if (!live(budget) || this.pending.size >= 64)
      throw new Error("RPC budget unavailable");
    const id = ++this.sequence;
    const response = new Promise<unknown>((resolve, reject) =>
      this.pending.set(id, { method, resolve, reject }),
    );
    // Attach the rejection observer before writing or waiting so exit never rejects unobserved.
    const result = bounded(response, budget);
    try {
      this.write({ id, method, params });
      return await result;
    } catch (error) {
      this.pending.get(id)?.reject(new Error("RPC unavailable"));
      await result.catch(() => {});
      if (error instanceof NativeNotSubmittedError) throw error;
      throw new Error("RPC unavailable");
    } finally {
      this.pending.delete(id);
    }
  }
  notify(method: string, params?: unknown): void {
    this.write({ method, ...(params === undefined ? {} : { params }) });
  }
  reply(id: string | number, result: unknown): void {
    this.write({ id, result });
  }
  reject(id: string | number): void {
    this.write({
      id,
      error: { code: -32601, message: "Unsupported native request" },
    });
  }
  listen(handler: (message: NativeMessage) => void): void {
    if (this.handler) throw new Error("native observer already installed");
    this.handler = handler;
    for (const message of this.buffered.splice(0)) handler(message);
  }
  private kill(signal: NodeJS.Signals): void {
    if (!this.child.pid || this.ended) return;
    try {
      if (process.platform === "win32") this.child.kill(signal);
      else process.kill(-this.child.pid, signal);
    } catch {
      /* Exit races are settled by the close event. */
    }
  }
  async close(budget: Budget): Promise<Result<{ processStopped: boolean }>> {
    this.closing = true;
    if (this.ended) return ok({ processStopped: true });
    if (!live(budget)) return fail("unavailable", "same_command");
    this.kill("SIGTERM");
    const started = Date.now();
    try {
      await bounded(this.stopped, {
        ...budget,
        timeoutMs: Math.max(1, Math.floor(budget.timeoutMs / 2)),
      });
    } catch {
      this.kill("SIGKILL");
      try {
        await bounded(this.stopped, {
          ...budget,
          timeoutMs: Math.max(1, budget.timeoutMs - (Date.now() - started)),
        });
      } catch {
        return fail("unavailable", "same_command");
      }
    }
    return ok({ processStopped: this.ended });
  }
}
export const nativeRuntime: RuntimeFactory = (spec) =>
  new StdioConnection(spec);
