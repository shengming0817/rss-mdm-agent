import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { UiProvider, StatePanel } from "@rss-mdm-agent/ui";
import { App } from "./App";
import { fixtureSelected, verifyAssembly } from "./assembly";
import "@rss-mdm-agent/ui/style.css";

const root = createRoot(document.getElementById("app")!);
async function main() {
  await verifyAssembly();
  if (import.meta.env.DEV && fixtureSelected) {
    const { FixtureApp } = await import("../../../tests/assistant/main");
    root.render(
      <StrictMode>
        <FixtureApp />
      </StrictMode>,
    );
  } else
    root.render(
      <StrictMode>
        <UiProvider>
          <App />
        </UiProvider>
      </StrictMode>,
    );
}
void main().catch(() => {
  root.render(
    <UiProvider>
      <main className="rss-page">
        <StatePanel
          kind="error"
          title="启动失败"
          description="无法核实当前运行装配，请检查本地服务后重试。"
        />
      </main>
    </UiProvider>,
  );
});
