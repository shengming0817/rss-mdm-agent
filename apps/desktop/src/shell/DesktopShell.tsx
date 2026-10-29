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
import { Button, Tooltip, type InputRef } from "antd";
import {
  MenuFoldOutlined,
  MenuUnfoldOutlined,
  UserOutlined,
} from "@ant-design/icons";
import type { Workspace } from "../state/workspace";
import { layouts, type PageId } from "../state/navigation";
import { useDesktop, usePlatform } from "../state/react";
import { WindowControls } from "./WindowControls";
import { nativeWindow } from "../workspace/window";
import { pageTitle } from "./navigation-model";
import {
  AppSettingsNavigation,
  HiddenSecondary,
  PrimaryNavigation,
} from "./NavigationMenu";
import { GlobalSearch } from "./GlobalSearch";
import { SparklesIcon } from "./SparklesIcon";
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
        if (result.kind === "localTool") {
          if (result.value.kind === "software" && result.value.software) {
            const original = result.value.software;
            if (
              !workspace.software
                .installed()
                .some(
                  (item) =>
                    JSON.stringify(item.reference) === JSON.stringify(original),
                )
            )
              await workspace.software.discover();
            if (
              !workspace.software
                .installed()
                .some(
                  (item) =>
                    JSON.stringify(item.reference) === JSON.stringify(original),
                )
            )
              return false;
          }
          if (
            result.value.kind === "network" &&
            result.value.target &&
            workspace.networkCheck.state.request &&
            !workspace.networkCheck.ended()
          )
            return false;
          workspace.navigation.stage(result.value);
          return true;
        }
        if (
          result.kind === "policy" &&
          result.value.task.kind !== "enrollment"
        ) {
          if (!workspace.policies.clear()) return false;
          await workspace.policies.select(result.value);
          if (!workspace.policies.state.item) return false;
          workspace.navigation.select(
            result.value.task.kind === "software" ? "software" : "tools",
          );
          workspace.navigation.showPolicy();
          return true;
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
  onAssistant,
  children,
  assistantTrigger,
  navToggleRef,
}: {
  assistantTrigger?: RefObject<HTMLButtonElement | null>;
  navToggleRef?: RefObject<HTMLButtonElement | null>;
  workspace: Workspace;
  onAssistant(): void;
  children: ReactNode;
}) {
  const desktop = useDesktop(),
    platform = usePlatform(),
    nav = useSnapshot(workspace.navigation.state),
    appearance = useSnapshot(desktop.appearance.state),
    assistant = useSnapshot(workspace.assistant.state);
  const controls = useMemo(nativeWindow, []),
    ownToggle = useRef<HTMLButtonElement>(null),
    searchInput = useRef<InputRef>(null);
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
      if (
        [
          ...document.querySelectorAll<HTMLElement>(
            '[role="dialog"][aria-modal="true"]',
          ),
        ].some(
          (dialog) =>
            dialog.getClientRects().length > 0 &&
            !dialog.closest('[hidden],[aria-hidden="true"]'),
        )
      )
        return;
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        workspace.navigation.setSearch(true);
        searchInput.current?.focus();
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
    const outsideSearch = (event: Event) => {
      if (
        !event
          .composedPath()
          .some(
            (node) =>
              node instanceof HTMLElement &&
              node.classList.contains("rss-top-search"),
          )
      )
        workspace.navigation.setSearch(false);
    };
    document.addEventListener("pointerdown", outsideSearch, true);
    document.addEventListener("focusin", outsideSearch);
    document.addEventListener("keydown", keyboard);
    return () => {
      narrow.removeEventListener("change", resize);
      document.removeEventListener("keydown", keyboard);
      document.removeEventListener("pointerdown", outsideSearch, true);
      document.removeEventListener("focusin", outsideSearch);
    };
  }, [workspace]);
  const user = workspace.identity.user,
    enterprise = workspace.identity.identity.mode === "user_device";
  const accountButton = (top = false) => (
    <button
      type="button"
      className={
        top ? "rss-account-button rss-account-top" : "rss-account-button"
      }
      aria-label="RSS MDM Agent 设置"
      aria-current={nav.page === "settings" ? "page" : undefined}
      onClick={() => workspace.navigation.select("settings")}
    >
      <span className="rss-avatar">
        {user.displayName.trim().slice(0, 1) || <UserOutlined />}
      </span>
      <span className="rss-account-copy">
        <strong>{user.displayName}</strong>
        <small>{enterprise ? "企业账号" : "本机工作区"}</small>
      </span>
    </button>
  );
  return (
    <div
      className="rss-desktop-shell"
      data-native-window={!!controls}
      data-layout={layout}
      data-layout-step={nav.layoutStep}
      data-material={appearance.materialEnabled}
      data-reduced-motion={appearance.reducedMotion}
      data-high-contrast={appearance.highContrast}
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
        <GlobalSearch workspace={workspace} input={searchInput} />
        <Tooltip title="页面助手">
          <Button
            ref={assistantTrigger}
            className="rss-page-assistant-trigger"
            type="text"
            icon={<SparklesIcon />}
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
        <div className="rss-sidebar-account">
          {nav.page === "settings" && (
            <AppSettingsNavigation workspace={workspace} />
          )}
          {accountButton()}
        </div>
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
  const [visited, setVisited] = useState(page === current);
  useEffect(() => {
    if (page === current) setVisited(true);
  }, [page, current]);
  return (
    <section
      className="rss-page-viewport"
      hidden={page !== current}
      aria-label={pageTitle(page)}
    >
      {visited || page === current ? children : null}
    </section>
  );
}
