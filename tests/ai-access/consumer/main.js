import {
  createSurfaceRenderer,
  RuntimeSurface,
} from "@rss-mdm-agent/ai-ui-bridge";
import { createApp } from "vue";
import {
  RuntimeClient,
  channelStream,
  ClientError,
} from "@rss-mdm-agent/ai-client";
import App from "./App.vue";
const runtime = new RuntimeClient(
  channelStream({
    async send(message) {
      const response = await fetch("/send", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(message),
      });
      if (!response.ok) throw new Error("test transport closed");
    },
    listen(receive, disconnect) {
      const stop = new AbortController();
      void (async () => {
        try {
          while (!stop.signal.aborted) {
            const response = await fetch("/receive", { signal: stop.signal });
            if (!response.ok) throw new Error("test transport closed");
            for (const message of await response.json()) receive(message);
            await new Promise((resolve) => setTimeout(resolve, 5));
          }
        } catch {
          if (!stop.signal.aborted) disconnect();
        }
      })();
      return () => stop.abort();
    },
  }),
);
await runtime.initialize();
await runtime.restore("session-1", 1);
window.consumer = {
  runtime,
  now: 0,
  createSurfaceRenderer,
  RuntimeSurface,
  createApp,
  ClientError,
};
createApp(App, { runtime }).mount("#app");
