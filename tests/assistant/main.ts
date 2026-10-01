// Test-only assembly of the product App. No alternate page or production transport fallback.
import { createApp, h, ref, onMounted } from "vue";
import {
  RuntimeClient,
  ClientError,
  channelStream,
} from "../../packages/ai-client/dist/index.js";
import App from "../../apps/desktop/src/App.vue";
import "../../packages/ui/dist/style.css";
import "../../apps/desktop/src/style.css";
import type { AssistantServices } from "../../apps/desktop/src/assistant/controller";
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
  async taskDetails(operationRequestId, signal) {
    const response = await fetch(
      `/__fixture/details?request=${encodeURIComponent(operationRequestId)}`,
      { signal },
    );
    if (!response.ok) throw new Error("fixture read denied");
    return response.json();
  },
};
import type { SelfServicePort } from "../../apps/desktop/src/self-service/types";
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
  cancel: (input) => rpc("cancel", input),
};
export function mountFixture() {
  createApp({
    setup() {
      const state = ref<{ scenario: string; scenarios: string[] }>({
        scenario: "running",
        scenarios: [],
      });
      onMounted(async () => {
        state.value = await rpc("state");
      });
      async function select(event: Event) {
        const scenario = (event.target as HTMLSelectElement).value;
        await rpc("scenario", { scenario });
        state.value = await rpc("state");
      }
      return () =>
        h(
          App,
          {
            assistantServices: services,
            selfServicePort,
            environment: {
              kind: "fixture",
              host: undefined,
              service: {
                async read() {
                  return (
                    await rpc<{
                      service: import("../../apps/desktop/src/settings/service-contract").ServiceView;
                    }>("state")
                  ).service;
                },
              },
            },
          },
          {
            "fixture-controls": () =>
              h("label", [
                " 场景 ",
                h(
                  "select",
                  {
                    value: state.value.scenario,
                    onChange: select,
                    "aria-label": "fixture 场景",
                  },
                  state.value.scenarios.map((value) =>
                    h("option", { value }, value),
                  ),
                ),
              ]),
          },
        );
    },
  }).mount("#app");
}
if (location.pathname.startsWith("/tests/assistant/")) mountFixture();

// Browser-only component stimuli share the product test page and workspace modules.
import {
  RuntimeSurface,
  createSurfaceRenderer,
} from "../../packages/ai-ui-bridge/dist/index.js";
Object.assign(window, {
  surfaceTest: {
    createApp,
    RuntimeSurface,
    createSurfaceRenderer,
    ClientError,
  },
});
