import { useSnapshot } from "valtio";
import { useDesktop } from "./state/react";
import { PageHeader, SectionCard, StatePanel } from "@rss-mdm-agent/ui";

/** EPIC base only. Business pages are connected by the following implementation issues. */
export function App() {
  const desktop = useDesktop();
  const state = useSnapshot(desktop.state);
  return (
    <main className="rss-page">
      <PageHeader title="RSS Agent" description="桌面自助服务" />
      <SectionCard>
        <StatePanel
          kind="readonly"
          title="桌面界面重建中"
          description={
            state.error ||
            (state.phase === "ready"
              ? "本机会话已读取，业务页面正在重建。"
              : state.phase === "unavailable"
                ? "当前浏览器无法访问本机认证服务。"
                : state.phase === "starting"
                  ? "正在读取本机会话与服务。"
                  : "请从账号入口建立本机会话。业务页面正在重建。")
          }
        />
      </SectionCard>
    </main>
  );
}
