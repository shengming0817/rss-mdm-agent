// Test-only assembly of the product App. No alternate page or production transport fallback.
import { createApp, h, ref, onMounted } from "vue";
import { ClientError } from "../../packages/ai-client/dist/index.js";
import { createFixturePorts } from "./ports";
import App from "../../apps/desktop/src/App.vue";
import "../../packages/ui/dist/style.css";
import "../../apps/desktop/src/style.css";
export function mountFixture() {
  const ports = createFixturePorts();
  const { rpc } = ports;
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
            ...ports,
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
