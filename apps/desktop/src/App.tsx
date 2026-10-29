import { useEffect, useRef, useState } from "react";
import { Alert, Button, Modal, Tag } from "antd";
import { useSnapshot } from "valtio";
import {
  DetailDrawer,
  PageHeader,
  SectionCard,
  StatePanel,
} from "@rss-mdm-agent/ui";
import { useDesktop, useWorkspace, usePlatform } from "./state/react";
import type { Workspace } from "./state/workspace";
import { modules, type PageId } from "./state/navigation";
import { DesktopShell, PageContainer } from "./shell/DesktopShell";
import { GlobalSearch } from "./shell/GlobalSearch";
import { PageAssistant } from "./shell/PageAssistant";
import { pageTitle, secondaryEntries } from "./shell/navigation-model";
import { nativeWindow } from "./workspace/window";
import { WindowControls } from "./shell/WindowControls";
import { HomePage } from "./home/HomePage";
import {
  AccountPage,
  BindingNotice,
  OrganizationManager,
} from "./account/AccountPage";
import { AppearancePage } from "./account/AppearancePage";
import { LoginDialog } from "./account/LoginDialog";
import { RegistrationPage } from "./account/RegistrationPage";
import { createRegistration } from "./account/registration";
import { pageContext } from "./workspace/page-context";
import { pageResources } from "./workspace/page-projection";
import { taskPhase, taskTitle } from "./task-center/controller";
/** Account dialogs survive transitions; every private business subtree is keyed to its subject. */
export function App() {
  const desktop = useDesktop(),
    workspace = useWorkspace(),
    [login, setLogin] = useState(false),
    [organizations, setOrganizations] = useState(false);
  return (
    <>
      {workspace ? (
        <WorkspaceView
          key={workspace.key}
          workspace={workspace}
          onLogin={() => setLogin(true)}
        />
      ) : (
        <StartupGate onLogin={() => setLogin(true)} />
      )}
      <LoginDialog
        open={login}
        onClose={() => setLogin(false)}
        onManage={() => {
          if (desktop.state.workspace)
            desktop.state.workspace.navigation.select("settings", "account");
          else setOrganizations(true);
        }}
      />
      <Modal
        open={organizations}
        title="组织连接"
        footer={null}
        onCancel={() => setOrganizations(false)}
        width={680}
        destroyOnHidden
      >
        <OrganizationManager />
      </Modal>
    </>
  );
}
function StartupGate({ onLogin }: { onLogin(): void }) {
  const desktop = useDesktop(),
    state = useSnapshot(desktop.state),
    account = useSnapshot(desktop.account),
    busy =
      account.busy || state.registrationPending || state.phase === "starting";
  return (
    <div className="rss-startup-gate" data-native-window={!!nativeWindow()}>
      <header className="rss-topbar" data-tauri-drag-region>
        <div className="rss-chrome-leading">
          <div className="rss-traffic-slot" />
          <div className="rss-brand" aria-label="RSS">
            <i>r.</i>
            <strong>RSS</strong>
          </div>
        </div>
        <span className="rss-grow" />
        <WindowControls />
      </header>
      <main className="rss-startup-card">
        <span className="rss-startup-mark">r.</span>
        <h1>你的工作，从这里开始。</h1>
        <p className="rss-muted">在自己的设备上，连接应用、工作与帮助。</p>
        <BindingNotice />
        {busy ? (
          <StatePanel
            kind="loading"
            title={
              state.registrationPending
                ? "正在核验设备注册"
                : "正在读取本机会话"
            }
          />
        ) : state.phase === "unavailable" ? (
          <StatePanel
            kind="readonly"
            title="请在 RSS Agent 桌面中打开"
            description="当前浏览器无法访问本机认证服务。"
          />
        ) : (
          <>
            <StatePanel
              kind={state.phase === "failed" ? "error" : "readonly"}
              title={
                state.phase === "failed" ? "本机会话读取失败" : "建立本机会话"
              }
              description={
                state.error || "可以登录企业账号，或回到本机工作区。"
              }
              onRead={
                state.phase === "failed"
                  ? () => void desktop.start()
                  : undefined
              }
            />
            <div className="rss-account-actions">
              <Button type="primary" onClick={onLogin}>
                登录企业账号
              </Button>
              <Button onClick={() => void desktop.guest()}>
                使用本机工作区
              </Button>
            </div>
          </>
        )}
        {state.registrationPending && (
          <Button
            onClick={() =>
              void desktop.service?.cancelRegistration().catch(() => {})
            }
          >
            停止等待注册
          </Button>
        )}
      </main>
    </div>
  );
}
function WorkspaceView({
  workspace,
  onLogin,
}: {
  workspace: Workspace;
  onLogin(): void;
}) {
  const desktop = useDesktop(),
    platform = usePlatform(),
    nav = useSnapshot(workspace.navigation.state),
    device = useSnapshot(workspace.device.state),
    system = useSnapshot(workspace.system.state),
    software = useSnapshot(workspace.software.state),
    tasks = useSnapshot(workspace.tasks.state),
    [registration, setRegistration] = useState<ReturnType<
      typeof createRegistration
    > | null>(null),
    [companion, setCompanion] = useState(false),
    assistantButton = useRef<HTMLButtonElement>(null),
    navButton = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    const owner = createRegistration(desktop.service);
    setRegistration(owner);
    void owner.refresh();
    return () => owner.dispose();
  }, [desktop, workspace]);
  const pages: PageId[] = [...modules.map(([id]) => id), "settings"];
  const contexts = pages.map((page) =>
    pageContext(
      page,
      pageTitle(page),
      [
        {
          id: nav.selected[page],
          title:
            secondaryEntries(page, platform).find(
              (entry) => entry.id === nav.selected[page],
            )?.title ?? pageTitle(page),
        },
      ],
      pageResources(
        workspace,
        page,
        nav.selected[page],
        device,
        system,
        software,
        tasks,
      ),
      workspace.key,
      system.now,
    ),
  );
  useEffect(() => {
    workspace.assistant.validatePageContexts(contexts);
  }, [workspace, contexts]);
  const openCompanion = () => {
    if (!companion) {
      const context = contexts.find(
        (value) => value.path[0] === pageTitle(nav.page),
      );
      if (context) workspace.assistant.stageContext(context);
    }
    setCompanion(!companion);
  };
  const closeCompanion = () => {
    setCompanion(false);
    assistantButton.current?.focus();
  };
  return (
    <DesktopShell
      workspace={workspace}
      onLogin={onLogin}
      onAssistant={openCompanion}
      assistantTrigger={assistantButton}
      navToggleRef={navButton}
    >
      <div className="rss-workspace-pages">
        {pages.map((page) => (
          <PageContainer key={page} page={page} current={nav.page}>
            {page === "home" ? (
              <HomePage workspace={workspace} registration={registration} />
            ) : page === "settings" ? (
              <SettingsContent
                workspace={workspace}
                section={nav.selected.settings}
                registration={registration}
                onLogin={onLogin}
              />
            ) : (
              <PendingModule page={page} section={nav.selected[page]} />
            )}
          </PageContainer>
        ))}
      </div>
      <PageAssistant
        workspace={workspace}
        open={companion}
        onClose={closeCompanion}
      />
      <GlobalSearch workspace={workspace} />
      <DetailDrawer
        open={nav.page === "tasks" && !!tasks.selected}
        onClose={() => workspace.tasks.closeTask()}
        returnFocus={() => navButton.current?.focus()}
        title={tasks.selected ? taskTitle(tasks.selected) : "原任务"}
        description="当前主体可读取的原任务记录"
      >
        <div className="rss-stack">
          {tasks.selected && (
            <>
              <Tag
                color={
                  tasks.selected.stage === "verified" ? "green" : "default"
                }
              >
                {taskPhase[tasks.selected.stage] ?? "状态待核实"}
              </Tag>
              <dl className="rss-account-facts">
                <dt>原任务引用</dt>
                <dd className="rss-reference">
                  {JSON.stringify(tasks.selected.reference)}
                </dd>
                <dt>来源</dt>
                <dd>
                  {tasks.selected.source === "human"
                    ? "人工"
                    : tasks.selected.source === "ai"
                      ? "AI"
                      : "策略"}
                </dd>
                <dt>记录时间</dt>
                <dd>
                  {tasks.selected.recordedAt != null
                    ? new Date(tasks.selected.recordedAt).toLocaleString()
                    : "尚未提供"}
                </dd>
              </dl>
              <p className="rss-muted">
                收到请求、执行结束、结果核实分别记录；这里不会重复提交原操作。
              </p>
              <Button
                onClick={() => {
                  if (workspace.tasks.state.selected)
                    void workspace.tasks.openTask(
                      workspace.tasks.state.selected.reference,
                    );
                }}
              >
                重新读取原任务
              </Button>
            </>
          )}
          {tasks.error && <Alert type="warning" title={tasks.error} />}
        </div>
      </DetailDrawer>
    </DesktopShell>
  );
}
function SettingsContent({
  workspace,
  section,
  registration,
  onLogin,
}: {
  workspace: Workspace;
  section: string;
  registration: ReturnType<typeof createRegistration> | null;
  onLogin(): void;
}) {
  return (
    <>
      {
        <div hidden={section !== "account"}>
          <AccountPage workspace={workspace} onLogin={onLogin} />
        </div>
      }
      <div hidden={section !== "registration"}>
        {registration ? (
          <RegistrationPage owner={registration} onLogin={onLogin} />
        ) : (
          <StatePanel kind="loading" title="正在读取设备注册" />
        )}
      </div>
      <div hidden={section !== "appearance"}>
        <AppearancePage workspace={workspace} />
      </div>
      {section !== "account" &&
        section !== "registration" &&
        section !== "appearance" && (
          <PendingModule page="settings" section={section} />
        )}
    </>
  );
}
function PendingModule({ page, section }: { page: PageId; section: string }) {
  const platform = usePlatform();
  return (
    <div className="rss-page">
      <PageHeader
        title={
          secondaryEntries(page, platform).find((entry) => entry.id === section)
            ?.title ?? pageTitle(page)
        }
      />
      <SectionCard>
        <StatePanel
          kind="readonly"
          title="当前页面暂不可用"
          description="可以返回主页查看设备状态，或使用页面助手描述问题。"
        />
      </SectionCard>
    </div>
  );
}
