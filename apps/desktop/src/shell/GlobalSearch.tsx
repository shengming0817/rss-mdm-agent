import {
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
  type RefObject,
} from "react";
import { Input, Popover, type InputRef } from "antd";
import { SearchOutlined, ArrowRightOutlined } from "@ant-design/icons";
import { useSnapshot } from "valtio";
import type { Workspace } from "../state/workspace";
import { usePlatform } from "../state/react";
import { moduleEntries, secondaryEntries } from "./navigation-model";
import { selectSection } from "./NavigationMenu";
import { taskKey, taskTitle } from "../task-center/controller";
import { policyPresentation } from "../software/presentation";
type Result = { id: string; title: string; group: string; choose(): void };
export function GlobalSearch({
  workspace,
  input,
}: {
  workspace: Workspace;
  input: RefObject<InputRef | null>;
}) {
  const nav = useSnapshot(workspace.navigation.state),
    software = useSnapshot(workspace.software.state),
    tasks = useSnapshot(workspace.tasks.state),
    directory = useSnapshot(workspace.directory.state),
    platform = usePlatform(),
    anchor = useRef<HTMLDivElement>(null),
    popup = useRef<HTMLDivElement>(null),
    list = useRef<HTMLDivElement>(null),
    listId = useId(),
    [query, setQuery] = useState(""),
    [selected, setSelected] = useState(0);
  const entries = useMemo(() => {
    const destinations = [
      ...moduleEntries,
      { id: "settings" as const, title: "RSS MDM Agent 设置" },
    ];
    const rows: Result[] = destinations.flatMap((module) => [
      {
        id: module.id,
        title: module.title,
        group: "页面",
        choose: () => workspace.navigation.select(module.id),
      },
      ...secondaryEntries(module.id, platform).map((section) => ({
        id: `${module.id}.${section.id}`,
        title: section.title,
        group: module.title,
        choose: () => {
          if (module.id === "assistant" && section.id === "history") {
            if (!workspace.navigation.state.historyOpen)
              workspace.navigation.toggleHistory();
            workspace.navigation.select("assistant", "history");
          } else selectSection(workspace, module.id, section.id);
        },
      })),
    ]);
    for (const item of workspace.software.summary(software).items)
      rows.push({
        id: `software:${item.reference.installation}`,
        title: item.name,
        group: "已读取的本机应用",
        choose: () => {
          workspace.navigation.select("software", "installed");
          workspace.navigation.locate({
            kind: "resource",
            page: "software",
            id: item.reference.installation,
            revision: JSON.stringify(item.reference),
          });
        },
      });
    for (const item of directory.items.filter(
      (value) => value.task.kind === "software" || value.task.kind === "script",
    )) {
      const view = policyPresentation(item),
        page = item.task.kind === "software" ? "software" : "tools";
      rows.push({
        id: `policy:${item.policyId}`,
        title: view.name,
        group: "已读取的组织发布目录",
        choose: () => {
          workspace.navigation.select(
            page,
            page === "software" ? "discover" : "all",
          );
          workspace.navigation.locate({
            kind: "resource",
            page,
            id: `policy:${item.policyId}`,
            revision: JSON.stringify([
              item.policyId,
              item.policyVersion,
              item.policyRevision,
              item.subject,
            ]),
          });
        },
      });
    }
    for (const item of workspace.tasks.recentItems(tasks))
      rows.push({
        id: `task:${taskKey(item.reference)}`,
        title: taskTitle(item),
        group: "最近任务",
        choose: () => {
          workspace.navigation.select("tasks", "history");
          void workspace.tasks.openTask(item.reference);
        },
      });
    return rows;
  }, [workspace, software, tasks, directory, platform]);
  const search = query.trim().toLocaleLowerCase(),
    matches = entries
      .filter(
        (row) =>
          !search ||
          `${row.title} ${row.group}`.toLocaleLowerCase().includes(search),
      )
      .slice(0, 30),
    active = Math.min(selected, Math.max(0, matches.length - 1));
  useEffect(() => {
    if (nav.searchOpen)
      list.current
        ?.querySelector('[aria-selected="true"]')
        ?.scrollIntoView({ block: "nearest" });
  }, [nav.searchOpen, query, active]);
  const close = () => workspace.navigation.setSearch(false),
    choose = (entry?: Result) => {
      if (entry) {
        entry.choose();
        input.current?.focus();
        close();
      }
    };
  useEffect(() => {
    if (!nav.searchOpen) return;
    const outside = (event: Event) => {
      const target = event.target;
      if (
        target instanceof Node &&
        !anchor.current?.contains(target) &&
        !popup.current?.contains(target)
      )
        workspace.navigation.setSearch(false);
    };
    document.addEventListener("pointerdown", outside, true);
    document.addEventListener("focusin", outside);
    return () => {
      document.removeEventListener("pointerdown", outside, true);
      document.removeEventListener("focusin", outside);
    };
  }, [workspace, nav.searchOpen]);
  return (
    <div ref={anchor} className="rss-top-search">
      <Popover
        open={nav.searchOpen}
        trigger={[]}
        placement="bottom"
        arrow={false}
        getPopupContainer={(node) => node.parentElement!}
        content={
          <div ref={popup} className="rss-search-popup">
            <div
              ref={list}
              className="rss-search-results"
              id={listId}
              role="listbox"
              aria-label="搜索结果"
            >
              {matches.length ? (
                matches.map((entry, index) => (
                  <button
                    type="button"
                    key={entry.id}
                    id={`${listId}-${index}`}
                    role="option"
                    aria-selected={active === index}
                    onMouseEnter={() => setSelected(index)}
                    onMouseDown={(event) => event.preventDefault()}
                    onClick={() => choose(entry)}
                  >
                    <span>
                      <strong>{entry.title}</strong>
                      <small>{entry.group}</small>
                    </span>
                    <ArrowRightOutlined />
                  </button>
                ))
              ) : (
                <p className="rss-muted">没有匹配的结果。可以换个关键词。</p>
              )}
            </div>
            <div className="rss-search-footer">
              <span>↑↓ 选择 · Enter 前往 · Esc 关闭</span>
              <span>本机应用与任务仅搜索当前主体已读取的记录</span>
            </div>
          </div>
        }
      >
        <Input
          ref={input}
          prefix={<SearchOutlined />}
          suffix={<kbd>{platform === "macos" ? "⌘" : "Ctrl"} K</kbd>}
          onFocus={() => workspace.navigation.setSearch(true)}
          onClick={() => workspace.navigation.setSearch(true)}
          role="combobox"
          aria-expanded={nav.searchOpen}
          aria-label="搜索应用、工具和设置"
          aria-controls={listId}
          aria-activedescendant={
            nav.searchOpen && matches[active]
              ? `${listId}-${active}`
              : undefined
          }
          value={query}
          allowClear
          maxLength={256}
          placeholder="搜索应用、工具和设置"
          onChange={(event) => {
            setQuery(event.target.value);
            setSelected(0);
            workspace.navigation.setSearch(true);
          }}
          onKeyDown={(event) => {
            if (event.nativeEvent.isComposing) {
              event.stopPropagation();
              return;
            }
            if (event.key === "Escape") {
              event.preventDefault();
              event.stopPropagation();
              close();
              return;
            }
            if (!nav.searchOpen) {
              if (event.key === "ArrowDown")
                workspace.navigation.setSearch(true);
              return;
            }
            if (event.key === "ArrowDown") {
              event.preventDefault();
              setSelected((active + 1) % Math.max(1, matches.length));
            }
            if (event.key === "ArrowUp") {
              event.preventDefault();
              setSelected(
                (active - 1 + Math.max(1, matches.length)) %
                  Math.max(1, matches.length),
              );
            }
            if (event.key === "Enter") {
              event.preventDefault();
              choose(matches[active]);
            }
          }}
        />
      </Popover>
    </div>
  );
}
