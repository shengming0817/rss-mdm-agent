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
} from "../../packages/ai-contract/dist/wire.js";
import { createLoginFixture } from "./login-fixture";
import type { FixtureEnvironment } from "../../apps/desktop/src/assembly";
import { fixtureLocalPorts } from "./local-ports";
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
      const reference = { kind: "request", request: requestId };
      const detail = await rpc<
        import("../../apps/desktop/src/self-service/types").TaskDetail
      >("task-detail", reference);
      const receipt = await rpc<
        import("../../apps/desktop/src/self-service/types").TaskControlReceipt
      >("task-control", {
        reference,
        id: crypto.randomUUID(),
        scope: detail.scope,
        expectedRevision: detail.revision,
        action: "cancel",
      });
      if (receipt.disposition !== "confirmed")
        throw Error("fixture cancellation unconfirmed");
    },
    async taskDetails(operationRequestId, signal) {
      if (signal.aborted) throw Error("fixture aborted");
      const result = await rpc<
        import("../../apps/desktop/src/self-service/types").TaskDetail
      >("task-detail", { kind: "request", request: operationRequestId });
      if (signal.aborted) throw Error("fixture aborted");
      return result;
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
    offers: () => rpc("offers"),
    policy_resolve: (input) => rpc("policy_resolve", input),
    policy_directory: (input) => rpc("policy_directory", input),
    policy_invoke: (input) => rpc("policy_invoke", input),
    policy_confirm: (input) => rpc("policy_confirm", input),
    policy_status: (input) => rpc("policy_status", input),
    observe_software: (input) => rpc("observe_software", input),
    execute: (input) => rpc("execute", input),
    confirm: (input) => rpc("confirm", input),
  };

  const environment: FixtureEnvironment = {
    kind: "fixture",
    local: {
      ...fixtureLocalPorts(),
      tasks: {
        list: (input) => rpc("task-list", input),
        detail: (input) => rpc("task-detail", input),
        control: (input) => rpc("task-control", input),
        controlStatus: (input) => rpc("task-control-status", input),
      },
    },
    account: createLoginFixture(() => rpc<UserContext>("identity")),
    device: {
      async read() {
        return {
          attributes: {
            computerName: "Studio Mac",
            ipAddresses: ["192.0.2.24"],
            loginUser: "studio",
          },
        };
      },
    },
    identity: { read: () => rpc<UserContext>("identity") },
    host: {
      read: () => rpc<HostStatus>("host"),
      restart: (generation) => rpc<HostStatus>("host-restart", { generation }),
    },
    service: {
      async read() {
        return (
          await rpc<{
            service: import("../../apps/desktop/src/settings/service-contract").ServiceView;
          }>("state")
        ).service;
      },
      registration: () =>
        rpc<
          import("../../apps/desktop/src/settings/service-contract").RegistrationStatus
        >("registration"),
      mdmStatus: () =>
        rpc<
          import("../../apps/desktop/src/settings/service-contract").MdmStatus
        >("mdm-registration"),
      importRegistration: async () => null,
      cancelRegistration: async () => {},
      requestRegistration: () =>
        rpc<
          import("../../apps/desktop/src/settings/service-contract").RegistrationStatus
        >("registration-request", {}),
    },
  };
  return { assistantServices: services, selfServicePort, environment, rpc };
}
