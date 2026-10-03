import {
  RuntimeClient,
  ClientError,
  channelStream,
} from "../../packages/ai-client/dist/index.js";
import type { AssistantServices } from "../../apps/desktop/src/assistant/controller";
import type { SelfServicePort } from "../../apps/desktop/src/self-service/types";
import type {
  HostStatus,
  UserContext,
} from "../../packages/ai-contract/dist/index.js";
import type { FixtureEnvironment } from "../../apps/desktop/src/assembly";
export function createFixturePorts() {
  const services: AssistantServices = {
    async connect(options, signal) {
      const peer = await (
        await fetch("/__fixture/connect", { method: "POST", signal })
      ).text();
      const runtime = new RuntimeClient(
        channelStream({
          async send(message) {
            const response = await fetch(`/__fixture/send?peer=${peer}`, {
              method: "POST",
              body: JSON.stringify(message),
            });
            if (!response.ok) throw new Error("fixture disconnected");
          },
          listen(receive, disconnect) {
            const abort = new AbortController();
            void (async () => {
              try {
                while (!abort.signal.aborted) {
                  const response = await fetch(
                    `/__fixture/receive?peer=${peer}`,
                    { signal: abort.signal },
                  );
                  if (!response.ok) throw new Error("fixture disconnected");
                  for (const message of await response.json()) receive(message);
                  await new Promise((resolve) => setTimeout(resolve, 10));
                }
              } catch {
                if (!abort.signal.aborted) disconnect();
              }
            })();
            return () => abort.abort();
          },
        }),
        options,
      );
      Object.assign(window, { assistantRuntime: runtime });
      return { runtime, mode: "s1" };
    },
    async confirmTask(task) {
      const { request, attempt, revision } = task;
      await rpc("confirm", { request, task: task.task, attempt, revision });
    },
    async cancelTask(requestId) {
      await rpc("cancel", { requestId });
    },
    async taskDetails(operationRequestId, signal) {
      const response = await fetch(
        `/__fixture/details?request=${encodeURIComponent(operationRequestId)}`,
        { signal },
      );
      if (!response.ok) throw new Error("fixture read denied");
      return response.json();
    },
  };

  async function rpc<T>(method: string, input?: unknown): Promise<T> {
    const response = await fetch(
      `/__fixture/${method}`,
      input === undefined
        ? undefined
        : {
            method: "POST",
            headers: { "content-type": "application/json" },
            body: JSON.stringify(input),
          },
    );
    if (!response.ok) throw new Error("fixture service unavailable");
    return response.json();
  }
  const selfServicePort: SelfServicePort = {
    snapshot: (input) => rpc("snapshot", input),
    execute: (input) => rpc("execute", input),
    confirm: (input) => rpc("confirm", input),
    cancel: (input) => rpc("cancel", input),
  };

  const environment: FixtureEnvironment = {
    kind: "fixture",
    identity: { read: () => rpc<UserContext>("identity") },
    host: {
      read: () => rpc<HostStatus>("host"),
      restart: (generation) => rpc<HostStatus>("host-restart", { generation }),
      export: () => rpc<boolean>("host-export"),
    },
    service: {
      async read() {
        return (
          await rpc<{
            service: import("../../apps/desktop/src/settings/service-contract").ServiceView;
          }>("state")
        ).service;
      },
    },
  };
  return { assistantServices: services, selfServicePort, environment, rpc };
}
