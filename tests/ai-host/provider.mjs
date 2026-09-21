import { activeStage } from "../../packages/ai-contract/dist/index.js";
import { randomUUID } from "node:crypto";
import { spawn } from "node:child_process";
import { appendFileSync } from "node:fs";
import { join } from "node:path";
import { fixtureSession } from "../../packages/ai-contract/dist/testing/index.js";
import { workspaceIdentity } from "../../packages/ai-contract/dist/session.js";

// Scripted model semantics inside a real OS process. This is not model-quality evidence.
export async function createProvider({ configuration, tools }) {
  const scenario =
    new URL(import.meta.url).searchParams.get("scenario") ??
    configuration.config.revision;
  const runs = new Map();
  let current,
    closed = false;
  const trace = (type, extra = {}) =>
    appendFileSync(
      join(configuration.workingDirectory, "trace.ndjson"),
      JSON.stringify({
        type,
        pid: process.pid,
        sessionId: configuration.namespace.sessionId,
        config: configuration.config,
        ...extra,
      }) + "\n",
    );
  trace("activate");
  const earlyTool = async () => {
    if (scenario !== "early_tool") return;
    const result = await tools.propose(
      { name: "early", arguments: {} },
      { timeoutMs: 1000, signal: new AbortController().signal },
    );
    trace("early-tool-result", { ok: result.ok });
  };
  await earlyTool();
  if (scenario === "unknown_grandchild") {
    const child = spawn(process.execPath, ["-e", "setInterval(()=>{},1000)"], {
      stdio: "ignore",
    });
    trace("grandchild", { childPid: child.pid });
  }
  if (scenario === "activation_block") while (true) {}
  const emit = (run, value) => {
    run.values.push({
      binding: { ...run.binding },
      commandId: run.command.commandId,
      attemptId: run.attempt.attemptId,
      ...value,
    });
    run.wake?.();
  };
  const terminal = (run) => {
    if (run.done) return;
    emit(run, {
      type: "event",
      body: {
        type: "terminal",
        outcome: run.cancelled ? "cancelled" : "completed",
      },
    });
    run.done = true;
    run.wake?.();
  };
  const open = (previous) => {
    current = {
      ...activeStage(fixtureSession()).binding,
      provider: configuration.provider,
      workspaceId: workspaceIdentity(configuration.workingDirectory),
      config: configuration.config,

      generation: randomUUID(),
      nativeSessionId: previous?.nativeSessionId ?? randomUUID(),
    };
    return {
      ok: true,
      value: {
        binding: current,
        capabilities: {
          ...activeStage(fixtureSession()).capabilities,
          continuation: "across_processes",
          steer: scenario === "steer" ? "supported" : "unsupported",
          tools: tools ? "host_mediated" : "disabled",
        },
      },
    };
  };
  return {
    async createSession() {
      await earlyTool();
      return open();
    },
    async resume(previous) {
      if (scenario === "restore_unavailable")
        return { ok: false, error: { code: "unavailable", retry: "never" } };
      return open(previous);
    },
    async dispatch(binding, command, attempt) {
      trace("dispatch", {
        commandId: command.commandId,
        input: command.input.type,
        attemptId: attempt.attemptId,
      });
      if (command.input.type === "cancel") {
        const run = runs.get(command.input.targetCommandId);
        if (!run)
          return {
            certainty: "not_sent",
            error: { code: "stale_binding", retry: "never" },
          };
        run.cancelled = true;
        setTimeout(() => terminal(run), 10);
        return {
          certainty: "acknowledged",
          binding,
          acknowledgement: { type: "cancel", confirmation: "request_only" },
        };
      }
      if (command.input.type === "respond")
        return {
          certainty: "acknowledged",
          binding,
          acknowledgement: { type: "respond" },
        };
      if (command.input.policy === "steer")
        return {
          certainty: "acknowledged",
          binding,
          acknowledgement: { type: "steer" },
        };
      if (scenario === "block") while (true) {}
      current = {
        ...binding,
        nativeRunId: randomUUID(),
        nativeRequestId: randomUUID(),
      };
      const run = {
        binding: current,
        command,
        attempt,
        values: [],
        done: false,
      };
      runs.set(command.commandId, run);
      if (scenario.startsWith("unknown"))
        return { certainty: "unknown", correlationId: "native-correlation" };
      emit(run, { type: "submitted" });
      if (command.input.text !== "queued-native")
        emit(run, { type: "running" });
      if (command.input.text === "question")
        emit(run, {
          type: "interaction",
          interaction: {
            category: "question",
            interactionId: "question-1",
            nativeCallbackId: "callback-1",
            expiresAtMs: Date.now() + 60000,
            callbackLifetime: "generation_bound",
            request: { question: "Continue?", options: ["yes", "no"] },
          },
        });
      if (tools)
        void tools
          .propose(
            { name: "fixture", arguments: {} },
            { timeoutMs: 1000, signal: new AbortController().signal },
          )
          .then((result) => trace("tool-result", { ok: result.ok }));
      if (command.input.text === "flood") {
        let count = 0;
        const timer = setInterval(() => {
          if (closed || run.done || count++ >= 500) {
            clearInterval(timer);
            return;
          }
          emit(run, {
            type: "delta",
            messageId: "flood",
            text: "x".repeat(8192),
          });
        }, 1);
      }
      if (
        command.input.text === "quick" ||
        command.input.text === "Reply with OK only. Do not use any tools."
      )
        setTimeout(() => {
          emit(run, {
            type: "event",
            body: { type: "text", messageId: "message", text: "completed" },
          });
          if (
            scenario === "probe_reject" &&
            command.input.text === "Reply with OK only. Do not use any tools."
          ) {
            emit(run, {
              type: "event",
              body: { type: "terminal", outcome: "failed" },
            });
            run.done = true;
            run.wake?.();
          } else terminal(run);
        }, 30);
      return { certainty: "submitted", binding: current };
    },
    async *observe(binding, budget) {
      const run =
        [...runs.values()].find(
          (r) => r.binding.nativeRunId === binding.nativeRunId,
        ) ?? [...runs.values()].at(-1);
      if (!run) return;
      const abort = () => run.wake?.();
      budget.signal.addEventListener("abort", abort);
      try {
        while (!closed && !budget.signal.aborted) {
          const event = run.values.shift();
          if (event) {
            yield event;
            continue;
          }
          if (run.done) return;
          await new Promise((resolve) => (run.wake = resolve));
        }
      } finally {
        budget.signal.removeEventListener("abort", abort);
      }
    },
    async reconcile(binding, record) {
      trace("reconcile", {
        commandId: record.command.commandId,
        attemptId: record.dispatch.attemptId,
      });
      return {
        ok: true,
        value: {
          binding: {
            ...binding,
            ...(record.dispatch.nativeRunId
              ? { nativeRunId: record.dispatch.nativeRunId }
              : {}),
            ...(record.dispatch.nativeRequestId
              ? { nativeRequestId: record.dispatch.nativeRequestId }
              : {}),
          },
          commandId: record.command.commandId,
          attemptId: record.dispatch.attemptId,
          status: "unknown",
        },
      };
    },
    async close() {
      closed = true;
      for (const run of runs.values()) run.wake?.();
      trace("close");
      return { ok: true, value: { processStopped: true } };
    },
  };
}
