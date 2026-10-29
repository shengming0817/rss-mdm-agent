import { useEffect, useRef, useState } from "react";
import { Button, Input, Tag } from "antd";
import { CloseOutlined, SearchOutlined } from "@ant-design/icons";
import { useSnapshot } from "valtio";
import { StatePanel } from "@rss-mdm-agent/ui";
import type { Workspace } from "../state/workspace";
import { operationMessage } from "./controller";
import { boundedText } from "../software/presentation";

/** Fixed, non-modal history; selecting a record never owns the conversation draft. */
export function SessionHistory({
  workspace,
  open,
}: {
  workspace: Workspace;
  open: boolean;
}) {
  const owner = workspace.assistant,
    state = useSnapshot(owner.state),
    [query, setQuery] = useState(""),
    root = useRef<HTMLElement>(null),
    focusFrame = useRef<number | undefined>(undefined),
    sessions = owner.sessions(state),
    visible = sessions.filter((session) =>
      `${session.presentation.title} ${session.namespace.sessionId}`
        .toLocaleLowerCase()
        .includes(query.trim().toLocaleLowerCase()),
    );
  useEffect(() => {
    if (!open) return;
    root.current?.querySelector("input")?.focus();
    if (state.connection === "connected") void owner.list(false);
  }, [open, owner, state.connection]);
  useEffect(
    () => () => {
      if (focusFrame.current !== undefined)
        cancelAnimationFrame(focusFrame.current);
    },
    [],
  );
  const close = () => {
    workspace.navigation.closeHistory();
    if (focusFrame.current !== undefined)
      cancelAnimationFrame(focusFrame.current);
    focusFrame.current = requestAnimationFrame(() => {
      focusFrame.current = undefined;
      if (!root.current?.isConnected) return;
      [
        ...document.querySelectorAll<HTMLButtonElement>(
          'button[aria-label="历史会话"]',
        ),
      ]
        .find((node) => node.getClientRects().length > 0)
        ?.focus();
    });
  };
  return (
    <aside
      ref={root}
      className="rss-session-history"
      aria-label="历史会话清单"
      hidden={!open}
    >
      <header>
        <h2>历史会话</h2>
        <Button
          type="text"
          icon={<CloseOutlined />}
          aria-label="关闭历史会话"
          onClick={close}
        />
      </header>
      <Input
        aria-label="搜索历史会话"
        prefix={<SearchOutlined />}
        placeholder="搜索已读取的会话"
        value={query}
        onChange={(event) => setQuery(event.target.value)}
        allowClear
      />
      <p className="rss-muted rss-history-coverage">
        当前搜索覆盖已读取的 {sessions.length} 条记录。
      </p>
      <div className="rss-session-history-list">
        {visible.map((session) => (
          <button
            type="button"
            className="rss-session-history-item"
            key={session.namespace.sessionId}
            aria-current={
              state.selected === session.namespace.sessionId
                ? "page"
                : undefined
            }
            onClick={() => void owner.select(session.namespace.sessionId)}
          >
            <strong>
              {boundedText(session.presentation.title, 160) || "未命名对话"}
            </strong>
            <span>
              {session.lastActivityAtMs > 0
                ? new Date(session.lastActivityAtMs).toLocaleString()
                : "时间未提供"}
            </span>
            {session.status === "retired" && <Tag>只读历史</Tag>}
          </button>
        ))}
        {!visible.length && !state.listing && (
          <StatePanel
            kind="empty"
            title={query ? "已读取记录中没有匹配会话" : "还没有已读取的会话"}
            description={
              state.next ? "可以继续读取其它记录。" : "从二级菜单开始新对话。"
            }
          />
        )}
        {state.listing && (
          <StatePanel kind="loading" title="正在读取会话记录" />
        )}
        {state.listError && (
          <StatePanel
            kind="error"
            title={operationMessage(state.listError)}
            onRead={() => void owner.list(!!state.next)}
          />
        )}
        {state.connection !== "connected" && (
          <p className="rss-muted">
            AI Host 未连接，当前只显示本次已加载的会话。
          </p>
        )}
        {state.next && (
          <Button
            block
            disabled={state.listing || state.connection !== "connected"}
            onClick={() => void owner.list(true)}
          >
            读取更多会话
          </Button>
        )}
      </div>
    </aside>
  );
}
