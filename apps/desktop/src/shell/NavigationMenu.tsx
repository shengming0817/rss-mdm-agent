import { Fragment } from "react";
import { Tooltip } from "antd";
import { useSnapshot } from "valtio";
import type { Workspace } from "../state/workspace";
import { usePlatform } from "../state/react";
import { moduleEntries, pageTitle, secondaryEntries } from "./navigation-model";
import type { PageId } from "../state/navigation";
export function selectSection(workspace: Workspace, page: PageId, id: string) {
  if (page === "assistant") {
    if (id === "new") workspace.assistant.create();
    if (id === "history") workspace.navigation.toggleHistory();
  }
  workspace.navigation.select(page, id);
}
function Secondary({ workspace }: { workspace: Workspace }) {
  const nav = useSnapshot(workspace.navigation.state),
    platform = usePlatform();
  return (
    <nav
      className="rss-secondary-nav"
      aria-label={`${pageTitle(nav.page)}二级菜单`}
    >
      {secondaryEntries(nav.page, platform).map(
        ({ id, title, Icon, group }) => (
          <Fragment key={id}>
            {group && <span className="rss-nav-group">{group}</span>}
            <Tooltip title={title} placement="right">
              <button
                type="button"
                className="rss-secondary-button"
                aria-label={title}
                aria-current={
                  nav.selected[nav.page] === id ? "page" : undefined
                }
                aria-expanded={
                  nav.page === "assistant" && id === "history"
                    ? nav.historyOpen
                    : undefined
                }
                onClick={() => selectSection(workspace, nav.page, id)}
              >
                <Icon />
                <span>{title}</span>
              </button>
            </Tooltip>
          </Fragment>
        ),
      )}
    </nav>
  );
}
export function PrimaryNavigation({
  workspace,
  inline = false,
}: {
  workspace: Workspace;
  inline?: boolean;
}) {
  const nav = useSnapshot(workspace.navigation.state);
  return (
    <nav
      className={
        inline ? "rss-primary-nav rss-primary-inline" : "rss-primary-nav"
      }
      aria-label="主导航"
    >
      {moduleEntries.map(({ id, title, Icon }) => (
        <div className="rss-module" key={id}>
          <Tooltip title={title} placement={inline ? "bottom" : "right"}>
            <button
              type="button"
              className="rss-primary-button"
              aria-label={title}
              aria-current={nav.page === id ? "page" : undefined}
              onClick={() => workspace.navigation.select(id)}
            >
              <Icon />
              <span>{title}</span>
            </button>
          </Tooltip>
          {!inline && nav.page === id && <Secondary workspace={workspace} />}
        </div>
      ))}
      {!inline && nav.page === "settings" && (
        <Secondary workspace={workspace} />
      )}
    </nav>
  );
}
export function HiddenSecondary({ workspace }: { workspace: Workspace }) {
  return <Secondary workspace={workspace} />;
}
