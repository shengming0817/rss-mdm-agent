import { useLayoutEffect, useRef } from "react";
import { Button } from "antd";
import { CloseOutlined, ExportOutlined } from "@ant-design/icons";
import { useSnapshot } from "valtio";
import { MessageComposer, MessageStream, StatePanel } from "@rss-mdm-agent/ui";
import type { Workspace } from "../state/workspace";
import { conversationMessages } from "../assistant/messages";
import { operationMessage } from "../assistant/controller";
import { copyText } from "../assistant/clipboard";
import { pageTitle } from "./navigation-model";
export function PageAssistant({
  workspace,
  open,
  onClose,
}: {
  workspace: Workspace;
  open: boolean;
  onClose(): void;
}) {
  const state = useSnapshot(workspace.assistant.state),
    nav = useSnapshot(workspace.navigation.state),
    root = useRef<HTMLElement>(null),
    view = workspace.assistant.view(state),
    messages = conversationMessages(view),
    error =
      state.errors.get(state.selected) || state.createError || state.error;
  useLayoutEffect(() => {
    if (open) root.current?.querySelector("textarea")?.focus();
  }, [open]);
  const pending = workspace.assistant.attention(state) > 0;
  return (
    <aside
      ref={root}
      className="rss-page-assistant"
      aria-label="页面助手"
      hidden={!open}
      onKeyDown={(event) => {
        if (event.key === "Escape") {
          event.stopPropagation();
          onClose();
        }
      }}
    >
      <header>
        <div>
          <h2>✧ 页面助手</h2>
          <p>当前页面：{pageTitle(nav.page)}</p>
        </div>
        <div>
          <Button
            type="text"
            icon={<ExportOutlined />}
            aria-label="在 AI 助手中继续"
            onClick={() => {
              workspace.navigation.select("assistant");
              onClose();
            }}
          />
          <Button
            type="text"
            icon={<CloseOutlined />}
            aria-label="关闭页面助手"
            onClick={onClose}
          />
        </div>
      </header>
      <div className="rss-page-assistant-messages">
        {messages.length ? (
          <MessageStream messages={messages} onCopy={copyText} />
        ) : (
          <div className="rss-companion-intro">
            <span className="rss-home-ai-symbol">✧</span>
            <h3>一起看看这个页面。</h3>
            <p>
              附上当前页面的展示信息，帮助说明问题。不会附带未提交的表单或私密字段。
            </p>
            {[
              "帮我解释当前页面的状态",
              "我应该先处理哪一项任务？",
              "帮我检查当前网络状态",
            ].map((text) => (
              <button
                type="button"
                key={text}
                onClick={() => workspace.assistant.draft.set(text)}
              >
                {text} <span>→</span>
              </button>
            ))}
          </div>
        )}
        {error && (
          <StatePanel
            kind={state.pending.has(state.selected) ? "unknown" : "error"}
            title={operationMessage(error)}
            onRead={
              state.selected
                ? () => void workspace.assistant.restore()
                : undefined
            }
          />
        )}
        {state.connection !== "connected" && (
          <StatePanel
            kind="readonly"
            title={
              state.connection === "connecting"
                ? "正在连接 AI Host"
                : "AI Host 未连接"
            }
            onRead={() => void workspace.assistant.connect()}
          />
        )}
        {pending && (
          <StatePanel
            kind="readonly"
            title="有待处理的确认或操作"
            description="请在 AI 助手中查看具体内容。"
            onRead={() => {
              workspace.navigation.select("assistant");
              onClose();
            }}
          />
        )}
      </div>
      <div className="rss-companion-composer">
        {workspace.assistant.context(state) && (
          <div className="rss-context-chip">
            <span>
              当前附加：{workspace.assistant.context(state)?.path.join(" / ")}
            </span>
            <Button
              type="text"
              size="small"
              aria-label="移除页面上下文"
              icon={<CloseOutlined />}
              onClick={() => workspace.assistant.removeContext()}
            />
          </div>
        )}
        {workspace.assistant.compositionError(state) && (
          <p className="rss-muted">
            {workspace.assistant.compositionError(state)}
          </p>
        )}
        <MessageComposer
          value={workspace.assistant.draft.get(state)}
          onChange={workspace.assistant.draft.set}
          onSend={() => void workspace.assistant.prompt()}
          canSend={
            workspace.assistant.canSend(state) &&
            !workspace.assistant.compositionError(state)
          }
          busy={state.sending.has(state.selected) || state.opening}
        />
        {workspace.assistant.canCancel(state) && (
          <Button
            type="text"
            size="small"
            onClick={() => void workspace.assistant.cancel()}
          >
            请求取消当前回复
          </Button>
        )}
      </div>
    </aside>
  );
}
