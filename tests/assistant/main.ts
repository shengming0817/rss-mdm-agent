// Test-only assembly of the product App. No alternate page or production transport fallback.
import { createApp, h, ref, onMounted } from "vue";
import { ClientError } from "../../packages/ai-client/dist/index.js";
import { createFixturePorts } from "./ports";
import OperationGallery from "./OperationGallery.vue";
import App from "../../apps/desktop/src/App.vue";
import "../../packages/ui/dist/style.css";
import "../../apps/desktop/src/style.css";
export function mountFixture() {
  const ports = createFixturePorts();
  const { rpc } = ports;
  createApp({
    setup() {
      const operationsEnabled = ref(false);
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
            "operation-cards": ({
              visible,
              drafts,
            }: {
              visible: boolean;
              drafts: OperationDrafts;
            }) =>
              visible && operationsEnabled.value
                ? h(OperationGallery, { drafts })
                : null,
            "fixture-controls": () => [
              h("label", [
                h("input", {
                  type: "checkbox",
                  "aria-label": "显示操作模板",
                  checked: operationsEnabled.value,
                  onChange: (e: Event) => {
                    operationsEnabled.value = (
                      e.target as HTMLInputElement
                    ).checked;
                  },
                }),
                "显示操作模板",
              ]),
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
            ],
          },
        );
    },
  }).mount("#app");
}
if (location.pathname.startsWith("/tests/assistant/")) mountFixture();

// Browser-only component stimuli share the product test page and workspace modules.
import {
  RuntimeSurface,
  type OperationDrafts,
  createSurfaceRenderer,
} from "../../packages/ai-ui-bridge/dist/index.js";
import { mountNode } from "../../apps/desktop/src/assistant/operation-ui/NodeView";
Object.assign(window, {
  surfaceTest: {
    createApp,
    mountNode,
    RuntimeSurface,
    createSurfaceRenderer,
    ClientError,
  },
});
