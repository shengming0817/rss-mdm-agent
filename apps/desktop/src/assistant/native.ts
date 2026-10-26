import { nativeTaskPort } from "../self-service/native";
import { userGeneration } from "../test-users";
import { invoke, isTauri } from "@tauri-apps/api/core";
import {
  channelStream,
  RuntimeClient,
  ClientError,
} from "@rss-mdm-agent/ai-client";
import type { AssistantServices } from "./controller";
/** IPC carries bounded ACP frames. Credentials, sockets and providers stay outside the WebView. */
export function nativeAssistant(
  taskCenter?: import("../task-center/controller").Controller,
): AssistantServices | undefined {
  if (!isTauri()) return undefined;
  const generation = userGeneration();
  const tasks = nativeTaskPort()!;
  return {
    async connect(options, signal) {
      const connectionId = await invoke<string>("ai_connect", { generation });
      let closed = false;
      let disconnected = () => {};
      const close = () => {
        if (closed) return;
        closed = true;
        signal.removeEventListener("abort", close);
        void invoke("ai_disconnect", { connectionId }).catch(() => {});
        disconnected();
      };
      signal.addEventListener("abort", close, { once: true });
      if (signal.aborted) {
        close();
        throw new ClientError("transport_closed");
      }
      const runtime = new RuntimeClient(
        channelStream({
          send: async (message) => {
            if (closed) throw new ClientError("transport_closed");
            await invoke("ai_send", { connectionId, message });
          },
          listen: (receive, disconnect) => {
            disconnected = disconnect;
            void (async () => {
              try {
                while (!closed) {
                  const message = await invoke<unknown>("ai_receive", {
                    connectionId,
                  });
                  if (!closed && message !== null) receive(message);
                }
              } catch {
                close();
              }
            })();
            return close;
          },
        }),
        options,
      );
      return { runtime, mode: "live" };
    },
    async submitOperation(input) {
      await invoke("ai_operation_submit", { generation, input });
      if (generation !== userGeneration())
        throw new ClientError("request_failed");
    },
    async reconcilePolicy(requestId) {
      await invoke("self_service_policy_status", {
        generation,
        input: { requestId },
      });
      if (generation !== userGeneration())
        throw new ClientError("request_failed");
    },
    async confirmPolicy(requestId) {
      await invoke("self_service_policy_confirm", {
        generation,
        input: { requestId },
      });
      if (generation !== userGeneration())
        throw new ClientError("request_failed");
    },
    async confirmTask(task) {
      const { request, attempt, revision } = task;
      await invoke("self_service_confirm", {
        generation,
        input: { request, task: task.task, attempt, revision },
      });
      if (generation !== userGeneration())
        throw new ClientError("request_failed");
    },
    async cancelTask(requestId) {
      const reference = { kind: "request" as const, request: requestId };
      if (taskCenter) {
        await taskCenter.cancel(requestId);
        return;
      }
      const detail = await tasks.detail(reference);
      const input = {
        reference,
        id: crypto.randomUUID(),
        scope: detail.scope,
        expectedRevision: detail.revision,
        action: "cancel" as const,
      };
      const receipt = await tasks.control(input);
      if (receipt.disposition !== "confirmed")
        throw new ClientError("request_failed");
    },
    async taskDetails(requestId, signal) {
      if (signal.aborted) throw new ClientError("request_failed");
      const result = taskCenter
        ? await taskCenter.detail({ kind: "request", request: requestId })
        : await tasks.detail({ kind: "request", request: requestId });
      if (generation !== userGeneration() || signal.aborted)
        throw new ClientError("request_failed");
      if (result.facts.kind === "execution")
        return { kind: "execution", value: result.facts.value };
      if (result.facts.kind === "preparation")
        return { kind: "pending", value: result.facts.value };
      throw new ClientError("request_failed");
    },
  };
}
