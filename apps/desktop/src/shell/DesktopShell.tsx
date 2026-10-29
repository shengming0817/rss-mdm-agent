import { Alert } from "antd";
import { createEntryNavigation } from "../workspace/entry-controller";
import { nativeNavigation, listenNavigation } from "../workspace/entry";
import type { NavigationOutput } from "../workspace/entry";
import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
  type RefObject,
} from "react";
import { useSnapshot } from "valtio";
import { Button, Dropdown, Tooltip } from "antd";
import {
  MenuFoldOutlined,
  MenuUnfoldOutlined,
  SearchOutlined,
  StarOutlined,
  UserOutlined,
} from "@ant-design/icons";
import type { Workspace } from "../state/workspace";
import { layouts, type PageId } from "../state/navigation";
import { useDesktop, usePlatform } from "../state/react";
import { WindowControls } from "./WindowControls";
import { nativeWindow } from "../workspace/window";
import { pageTitle } from "./navigation-model";
import { HiddenSecondary, PrimaryNavigation } from "./NavigationMenu";
import "./shell.css";
function EntryFeedback({
  entries,
}: {
  entries: ReturnType<typeof createEntryNavigation>;
}) {
  const state = useSnapshot(entries.state);
  return state.notice ? (
    <Alert
      type="warning"
      showIcon
      title={state.notice}
      description={
        <div>
          <Button size="small" onClick={() => void entries.retry()}>
            重试入口
          </Button>
          <Button size="small" onClick={() => void entries.dismiss()}>
            忽略
          </Button>
        </div>
      }
    />
  ) : null;
}
function NativeEntryObserver({ workspace }: { workspace: Workspace }) {
  const desktop = useDesktop(),
    [entries, setEntries] = useState<ReturnType<
      typeof createEntryNavigation
    > | null>(null);
  useEffect(() => {
    const owner = createEntryNavigation(
      desktop.fixture ? undefined : nativeNavigation(),
      desktop.fixture ? undefined : listenNavigation,
      async (result: NavigationOutput) => {
        if (
          result.kind === "event" &&
          result.value.selector.kind === "node" &&
          result.value.action === "view"
        ) {
          const [page, section] = result.value.selector.id.split(".");
          if (
            ![
              "home",
              "assistant",
              "software",
              "tools",
              "system",
              "tasks",
              "settings",
            ].includes(page)
          )
            return false;
          return workspace.navigation.select(page as PageId, section);
        }
        if (result.kind === "task") {
          workspace.navigation.select("tasks", "history");
          return (await workspace.tasks.openTask(result.value)) === "ready";
        }
        return false;
      },
    );
    setEntries(owner);
    void owner.start();
    return () => owner.dispose();
  }, [desktop, workspace]);
  return entries ? <EntryFeedback entries={entries} /> : null;
}
export function DesktopShell({
  workspace,
  onLogin,
  onAssistant,
  children,
  assistantTrigger,
  navToggleRef,
}: {
  assistantTrigger?: RefObject<HTMLButtonElement | null>;
  navToggleRef?: RefObject<HTMLButtonElement | null>;
  workspace: Workspace;
  onLogin(): void;
  onAssistant(): void;
  children: ReactNode;
}) {
  const desktop = useDesktop(),
    platform = usePlatform(),
    nav = useSnapshot(workspace.navigation.state),
    appearance = useSnapshot(desktop.appearance.state),
    assistant = useSnapshot(workspace.assistant.state);
  const controls = useMemo(nativeWindow, []),
    ownToggle = useRef<HTMLButtonElement>(null);
  const toggle = navToggleRef ?? ownToggle;
  const layout = layouts[nav.layoutStep];
  const collapse = () => {
    workspace.navigation.collapse();
    toggle.current?.focus();
  };
  useEffect(() => {
    const narrow = window.matchMedia("(max-width:960px)");
    const resize = () => {
      if (narrow.matches && workspace.navigation.state.layoutStep === 2)
        workspace.navigation.collapse();
    };
    resize();
    narrow.addEventListener("change", resize);
    const keyboard = (event: KeyboardEvent) => {
      const target = event.target;
      if (document.querySelector('[role="dialog"][aria-modal="true"]')) return;
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        workspace.navigation.setSearch(true);
      }
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "b") {
        if (
          target instanceof HTMLElement &&
          (target.isContentEditable || target.matches("input,textarea"))
        )
          return;
        event.preventDefault();
        workspace.navigation.hideOrRestore();
        toggle.current?.focus();
      }
      if (
        event.key === "Escape" &&
        narrow.matches &&
        workspace.navigation.state.layoutStep === 2
      ) {
        event.preventDefault();
        workspace.navigation.collapse();
        toggle.current?.focus();
      }
    };
    document.addEventListener("keydown", keyboard);
    return () => {
      narrow.removeEventListener("change", resize);
      document.removeEventListener("keydown", keyboard);
    };
  }, [workspace]);
  const user = workspace.identity.user,
    enterprise = workspace.identity.identity.mode === "user_device";
  const menu = {
    items: [
      { key: "account", label: "账号与组织" },
      { key: "registration", label: "设备注册与 MDM" },
      { key: "appearance", label: "外观与体验" },
      { type: "divider" as const },
      { key: "login", label: enterprise ? "切换账号" : "登录企业账号" },
      { key: "logout", label: enterprise ? "退出企业账号" : "刷新本机会话" },
    ],
    onClick: ({ key }: { key: string }) => {
      if (key === "login") onLogin();
      else if (key === "logout") void desktop.logout();
      else workspace.navigation.select("settings", key);
    },
  };
  const accountButton = (top = false) => (
    <Dropdown menu={menu} trigger={["click"]}>
      <button
        type="button"
        className={
          top ? "rss-account-button rss-account-top" : "rss-account-button"
        }
        aria-label="用户菜单"
      >
        <span className="rss-avatar">
          {user.displayName.trim().slice(0, 1) || <UserOutlined />}
        </span>
        <span className="rss-account-copy">
          <strong>{user.displayName}</strong>
          <small>{enterprise ? "企业账号" : "本机工作区"}</small>
        </span>
      </button>
    </Dropdown>
  );
  return (
    <div
      className="rss-desktop-shell"
      data-native-window={!!controls}
      data-layout={layout}
      data-layout-step={nav.layoutStep}
      data-material={appearance.materialEnabled}
    >
      <header className="rss-topbar" data-tauri-drag-region>
        <div className="rss-chrome-leading" data-tauri-drag-region>
          <div className="rss-traffic-slot" aria-hidden="true">
            {!controls && platform === "macos" && (
              <span className="rss-preview-traffic">
                <i />
                <i />
                <i />
              </span>
            )}
          </div>
          <div className="rss-brand" aria-label="RSS">
            <i aria-hidden="true">r.</i>
            <strong>RSS</strong>
          </div>
          <Tooltip
            title={
              nav.layoutStep === 1
                ? "展开侧栏"
                : nav.layoutStep === 2
                  ? "收起侧栏"
                  : nav.layoutStep === 3
                    ? "隐藏侧栏"
                    : "显示侧栏"
            }
          >
            <Button
              ref={toggle}
              className="rss-nav-toggle"
              type="text"
              aria-label="切换侧栏布局"
              aria-expanded={layout === "full"}
              onClick={() => workspace.navigation.cycle()}
              icon={
                nav.layoutStep === 1 || nav.layoutStep === 0 ? (
                  <MenuUnfoldOutlined />
                ) : (
                  <MenuFoldOutlined />
                )
              }
            />
          </Tooltip>
        </div>
        <span className="rss-top-label">{pageTitle(nav.page)}</span>
        <div className="rss-hidden-primary">
          <PrimaryNavigation workspace={workspace} inline />
        </div>
        <button
          type="button"
          className="rss-top-search"
          onClick={() => workspace.navigation.setSearch(true)}
          aria-label="搜索应用、设置与任务"
        >
          <SearchOutlined />
          <span>搜索应用、设置与任务</span>
          <kbd>{platform === "macos" ? "⌘" : "Ctrl"} K</kbd>
        </button>
        <Tooltip title="页面助手">
          <Button
            ref={assistantTrigger}
            className="rss-page-assistant-trigger"
            type="text"
            icon={<StarOutlined />}
            aria-label="页面助手"
            onClick={onAssistant}
          >
            <span>页面助手</span>
          </Button>
        </Tooltip>
        <div className="rss-hidden-account">{accountButton(true)}</div>
        <WindowControls />
      </header>
      <button
        type="button"
        className="rss-navigation-mask"
        aria-label="收起导航"
        onClick={collapse}
      />
      <aside className="rss-sidebar">
        <PrimaryNavigation workspace={workspace} />
        <div className="rss-sidebar-account">{accountButton()}</div>
      </aside>
      <div className="rss-hidden-secondary">
        <HiddenSecondary workspace={workspace} />
      </div>
      <main className="rss-shell-main">
        <NativeEntryObserver workspace={workspace} />
        {children}
      </main>
      <footer className="rss-shell-footer">
        <span>
          <i
            className={
              assistant.connection === "connected"
                ? "rss-status-dot connected"
                : "rss-status-dot"
            }
          />
          {assistant.connection === "connected"
            ? "AI Host 已连接"
            : assistant.connection === "connecting"
              ? "正在连接 AI Host"
              : "AI Host 未连接"}
        </span>
        <span>
          {enterprise ? "企业会话" : "本机工作区"} ·{" "}
          {platform === "macos" ? "macOS" : "Windows"}
        </span>
      </footer>
    </div>
  );
}
/** Page elements retain their own scroll and local draft state through chrome changes. */
export function PageContainer({
  page,
  current,
  children,
}: {
  page: PageId;
  current: PageId;
  children: ReactNode;
}) {
  return (
    <section
      className="rss-page-viewport"
      hidden={page !== current}
      aria-label={pageTitle(page)}
    >
      {children}
    </section>
  );
}
