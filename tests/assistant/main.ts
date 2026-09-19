// Test-only assembly of the product App. No alternate page or production transport fallback.
import { createApp } from "vue";
import {
  RuntimeClient,
  channelStream,
} from "../../packages/ai-client/dist/index.js";
import App from "../../apps/desktop/src/App.vue";
import "../../packages/ui/dist/style.css";
import "../../apps/desktop/src/style.css";
import type { AssistantServices } from "../../apps/desktop/src/assistant/controller";
const services: AssistantServices = {
  async connect(options) {
    const peer = await (
      await fetch("/__fixture/connect", { method: "POST" })
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
  async taskDetails(operationRequestId) {
    const response = await fetch(
      `/__fixture/details?request=${encodeURIComponent(operationRequestId)}`,
    );
    if (!response.ok) throw new Error("fixture read denied");
    return response.json();
  },
};
createApp(App, { assistantServices: services }).mount("#app");
