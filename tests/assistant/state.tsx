import { StrictMode, useMemo } from "react";
import { useSnapshot } from "valtio";
import { Button, Input } from "antd";
import { PageHeader, SectionCard } from "@rss-mdm-agent/ui";
import {
  DesktopProvider,
  useDesktop,
  useWorkspace,
} from "../../apps/desktop/src/state/react";
import { layouts } from "../../apps/desktop/src/state/navigation";
import type { Workspace } from "../../apps/desktop/src/state/workspace";
import { createFixturePorts } from "./ports";
/** A test consumer of the same owner and ports used by the application. */
function WorkspaceProbe({ workspace }: { workspace: Workspace }) {
  const assistant = useSnapshot(workspace.assistant.state),
    navigation = useSnapshot(workspace.navigation.state),
    software = useSnapshot(workspace.software.state);
  return (
    <SectionCard title="业务状态接缝">
      <p data-testid="state-connection">{assistant.connection}</p>
      <p data-testid="state-layout">
        {navigation.layoutStep}:{layouts[navigation.layoutStep]}
      </p>
      <p data-testid="state-page">
        {navigation.page}:{navigation.selected[navigation.page]}
      </p>
      <p data-testid="state-inventory">
        {workspace.software.summary(software).installed}
      </p>
      <p data-testid="state-sessions">
        {workspace.assistant.sessions(assistant).length}
      </p>
      <Input
        aria-label="状态层草稿"
        value={workspace.assistant.draft.get(assistant)}
        onChange={(event) => workspace.assistant.draft.set(event.target.value)}
      />
      <Button onClick={() => workspace.navigation.cycle()}>切换布局</Button>
      <Button
        onClick={() => workspace.navigation.select("software", "updates")}
      >
        选择软件更新
      </Button>
      <Button onClick={() => workspace.navigation.select("assistant")}>
        回到助手
      </Button>
      <Button onClick={() => void workspace.software.discover()}>
        读取软件
      </Button>
    </SectionCard>
  );
}
function OwnerProbe() {
  const desktop = useDesktop(),
    workspace = useWorkspace(),
    state = useSnapshot(desktop.state);
  return (
    <main className="rss-page rss-stack">
      <PageHeader
        title="业务状态验收"
        description="显式开发装配 · 真实客户端与测试 Host 接缝"
      />
      <p data-testid="state-owner">{state.phase}</p>
      {workspace && <WorkspaceProbe workspace={workspace} />}
    </main>
  );
}
export function StateFixture() {
  const input = useMemo(() => createFixturePorts(), []);
  return (
    <StrictMode>
      <DesktopProvider input={input}>
        <OwnerProbe />
      </DesktopProvider>
    </StrictMode>
  );
}
