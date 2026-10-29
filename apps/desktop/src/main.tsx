import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { UiProvider, StatePanel } from "@rss-mdm-agent/ui";
import { App } from "./App";
import { DesktopProvider } from "./state/react";
import { fixtureSelected, verifyAssembly } from "./assembly";
import "@rss-mdm-agent/ui/style.css";

const root = createRoot(document.getElementById("app")!);
let startupPhase: "assembly" | "presentation" = "assembly";
async function main() {
  await verifyAssembly();
  startupPhase = "presentation";
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
        <DesktopProvider>
          <App />
        </DesktopProvider>
      </StrictMode>,
    );
}
void main().catch((error: unknown) => {
  const mismatch =
    error instanceof Error && error.message === "desktop assembly mismatch";
  const code = mismatch
    ? "assembly_mismatch"
    : startupPhase === "assembly"
      ? "assembly_unavailable"
      : "presentation_load";
  const description = mismatch
    ? "桌面运行模式与前端配置不一致，请关闭窗口并以同一运行模式重新启动。"
    : startupPhase === "assembly"
      ? "无法读取桌面运行模式，请确认桌面进程可用后重新启动。"
      : "界面资源加载失败，请重新启动；若仍失败，请修复或重新安装应用。";
  // Keep a closed diagnostic code without exposing native payloads or credentials.
  console.error(`RSS_STARTUP ${code}`);
  root.render(
    <UiProvider>
      <main className="rss-page">
        <StatePanel kind="error" title="启动失败" description={description} />
      </main>
    </UiProvider>,
  );
});
