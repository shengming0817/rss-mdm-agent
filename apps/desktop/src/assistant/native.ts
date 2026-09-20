import { userGeneration } from "../test-users";
import { invoke, isTauri } from "@tauri-apps/api/core";
import {
  channelStream,
  RuntimeClient,
  ClientError,
} from "@rss-mdm-agent/ai-client";
import type { AssistantServices } from "./controller";
/** IPC carries bounded ACP frames. Credentials, sockets and providers stay outside the WebView. */
export function nativeAssistant(): AssistantServices | undefined {
  if (!isTauri()) return undefined;
  const generation = userGeneration();
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
      return { runtime, mode: "s1" };
    },
    async taskDetails(requestId, signal) {
      if (signal.aborted) throw new ClientError("request_failed");
      const result = await invoke<
        import("./execution-types").ExecutionTaskDetails
      >("execution_task_details", { requestId, generation });
      if (generation !== userGeneration())
        throw new ClientError("request_failed");
      return result;
    },
  };
}
