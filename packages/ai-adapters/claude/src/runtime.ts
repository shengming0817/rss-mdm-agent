import { spawn } from "node:child_process";
import {
  query,
  type Options,
  type Query,
  type SDKUserMessage,
} from "@anthropic-ai/claude-agent-sdk";
import { deferred } from "./support.js";
/** Narrow testing seam; production always uses the official full SDK. */
export type RuntimeFactory = (
  options: Options,
  prompts: AsyncIterable<SDKUserMessage>,
) => { query: Query; stopped: Promise<void> };
export const nativeRuntime: RuntimeFactory = (options, prompts) => {
  const stopped = deferred<void>();
  // ref: @anthropic-ai/claude-agent-sdk@0.3.277 sdk.d.ts SpawnOptions/SpawnedProcess.
  const native = query({
    prompt: prompts,
    options: {
      ...options,
      spawnClaudeCodeProcess: (spec) => {
        const child = spawn(spec.command, spec.args, {
          cwd: spec.cwd,
          env: spec.env,
          signal: spec.signal,
          stdio: ["pipe", "pipe", "pipe"],
          windowsHide: true,
        });
        child.stderr.resume(); // SDK diagnostics can contain prompts/credentials; never forward them.
        child.once("exit", () => stopped.resolve());
        child.once("error", () => {
          if (!child.pid) stopped.resolve();
        });
        return child;
      },
    },
  });
  return { query: native, stopped: stopped.promise };
};
