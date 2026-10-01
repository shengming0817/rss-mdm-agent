import { createApp, h } from "vue";
import App from "./App.vue";
import { fixtureSelected, verifyAssembly } from "./assembly";
import "@rss-mdm-agent/ui/style.css";
import "./style.css";
async function main() {
  await verifyAssembly();
  if (import.meta.env.DEV && fixtureSelected) {
    const { mountFixture } = await import("../../../tests/assistant/main");
    mountFixture();
  } else createApp(App).mount("#app");
}
void main().catch((error) => {
  createApp({
    render: () => h("p", { role: "alert" }, `启动失败：${error.message}`),
  }).mount("#app");
});
