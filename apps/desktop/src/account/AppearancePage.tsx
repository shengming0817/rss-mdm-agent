import { Button, Select, Tag } from "antd";
import { useSnapshot } from "valtio";
import { PageHeader, SectionCard } from "@rss-mdm-agent/ui";
import { useDesktop } from "../state/react";
import type { Workspace } from "../state/workspace";
export function AppearancePage({ workspace }: { workspace: Workspace }) {
  const desktop = useDesktop(),
    state = useSnapshot(desktop.state),
    appearance = useSnapshot(desktop.appearance.state);
  return (
    <div className="rss-page">
      <PageHeader
        title="外观与体验"
        description="选择界面主题，查看当前系统的显示偏好。"
      />
      <div className="rss-stack">
        <SectionCard title="界面与导航">
          <div className="rss-organization-row">
            <div className="rss-grow">
              <strong>界面主题</strong>
              <p className="rss-muted">即时应用于当前窗口</p>
            </div>
            <Select
              aria-label="界面主题"
              value={state.theme}
              options={[
                { value: "system", label: "跟随系统" },
                { value: "light", label: "浅色" },
                { value: "dark", label: "深色" },
              ]}
              onChange={desktop.setTheme}
              style={{ minWidth: 120 }}
            />
          </div>
          <div className="rss-organization-row">
            <div className="rss-grow">
              <strong>导航布局</strong>
              <p className="rss-muted">
                无侧栏、图标、完整、图标依次切换；⌘ / Ctrl B 隐藏或恢复。
              </p>
            </div>
            <Button onClick={workspace.navigation.cycle}>切换导航布局</Button>
          </div>
        </SectionCard>
        <SectionCard title="系统显示偏好">
          <p className="rss-muted">由本机系统提供；系统偏好变化后自动重读。</p>
          <div className="rss-organization-row">
            <strong className="rss-grow">减少动态效果</strong>
            <Tag>{appearance.reducedMotion ? "系统已开启" : "系统未开启"}</Tag>
          </div>
          <div className="rss-organization-row">
            <strong className="rss-grow">高对比度</strong>
            <Tag>{appearance.highContrast ? "系统已开启" : "系统未开启"}</Tag>
          </div>
          <div className="rss-organization-row">
            <strong className="rss-grow">窗口材质</strong>
            <Tag>{appearance.materialEnabled ? "系统材质" : "纯色背景"}</Tag>
          </div>
        </SectionCard>
      </div>
    </div>
  );
}
