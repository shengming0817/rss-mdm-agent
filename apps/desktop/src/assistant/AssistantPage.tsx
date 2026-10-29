import { useSnapshot } from "valtio";
import { Button } from "antd";
import { CloseOutlined } from "@ant-design/icons";
import { MessageComposer, StatePanel } from "@rss-mdm-agent/ui";
import type { Workspace } from "../state/workspace";
import { SessionHistory } from "./SessionHistory";
import { conversationMessages } from "./messages";
import { operationMessage } from "./controller";
import { AssistantTimeline } from "./AssistantTimeline";
import { useConversationScroll } from "./scroll";
import "./assistant.css";

/** Conversation presentation consumes the existing authenticated controller. */
export function AssistantPage({ workspace }: { workspace: Workspace }) {
  const owner = workspace.assistant,
    state = useSnapshot(owner.state),
    nav = useSnapshot(workspace.navigation.state),
    view = owner.view(state),
    messages = conversationMessages(view),
    error =
      state.errors.get(state.selected) || state.createError || state.error,
    context = owner.context(state),
    cancelling = owner.canCancel(state),
    pending = state.pending.has(state.selected),
    stream = useConversationScroll(
      state.selected,
      `${view?.cursor ?? 0}:${messages.map((message) => `${message.id}:${message.text.length}`).join(";")}`,
      nav.page === "assistant",
    );
  return (
    <div
      className={`rss-assistant-layout ${nav.historyOpen ? "rss-assistant-history-open" : ""}`}
    >
      <SessionHistory workspace={workspace} open={nav.historyOpen} />
      <section className="rss-assistant-conversation" aria-label="AI 会话">
        <div
          ref={stream.viewport}
          className="rss-assistant-scroll"
          tabIndex={0}
          aria-label="会话内容"
        >
          <div ref={stream.content} className="rss-assistant-thread">
            {view?.timeline.length ? (
              <AssistantTimeline
                workspace={workspace}
                active={nav.page === "assistant"}
              />
            ) : (
              <div className="rss-assistant-intro">
                <span aria-hidden="true">✧</span>
                <h1>一起，把事情做得更好。</h1>
                <p>描述你想了解或处理的事情。具体操作仍需核对原准备和权限。</p>
                <div>
                  {[
                    "帮我看看设备的当前状态",
                    "查找适合工作的应用",
                    "解释一项任务的进度",
                  ].map((text) => (
                    <button
                      type="button"
                      key={text}
                      onClick={() => owner.draft.set(text)}
                    >
                      {text}
                      <span aria-hidden="true">↗</span>
                    </button>
                  ))}
                </div>
              </div>
            )}
            {error && (
              <StatePanel
                kind={pending ? "unknown" : "error"}
                title={operationMessage(error)}
                onRead={state.selected ? () => void owner.restore() : undefined}
              />
            )}
            {pending && (
              <StatePanel
                kind="unknown"
                title="原消息回执尚未确认"
                description="先读取原对话，再恢复同一请求；不会重新生成发送身份。"
                onRead={() => void owner.retry()}
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
                description="本次已加载的文本仍可阅读，草稿保留在当前账号工作区。"
                onRead={() => void owner.connect()}
              />
            )}
            {state.connection === "connected" &&
              !owner.connectionReady(state) && (
                <StatePanel
                  kind="readonly"
                  title="当前没有可用的 AI 连接"
                  description="在连接设置中核对并测试连接后继续。"
                >
                  <Button
                    onClick={() =>
                      workspace.navigation.select("settings", "connection")
                    }
                  >
                    打开连接设置
                  </Button>
                </StatePanel>
              )}
            {state.cleanupError && (
              <StatePanel
                kind="unknown"
                title="前次 AI 资源清理尚未确认"
                description="不能据此确认模型已停止或设备任务已取消；请先核对原运行状态。"
              >
                <Button
                  onClick={() =>
                    workspace.navigation.select("settings", "diagnostics")
                  }
                >
                  打开运行与诊断
                </Button>
              </StatePanel>
            )}
            {view?.sessionStatus === "retired" && (
              <StatePanel
                kind="readonly"
                title="这条会话已退休，仅可阅读"
                description="从二级菜单开始新对话；历史与草稿不会自动带入。"
              />
            )}
            {view?.sessionStatus === "recovery_required" && (
              <StatePanel
                kind="readonly"
                title="原模型上下文需要恢复"
                description="读取会话只恢复历史视图。可在应用设置的 AI 与连接 → 会话选项中，核对恢复能力或明确开始新上下文。"
                onRead={() => void owner.restore()}
              >
                <Button
                  onClick={() =>
                    workspace.navigation.select("settings", "connection")
                  }
                >
                  打开连接设置
                </Button>
              </StatePanel>
            )}
            {state.optionPending.has(state.selected) && (
              <StatePanel
                kind="unknown"
                title="原会话操作回执尚未确认"
                description="先读取同一会话，核对连接与上下文状态，不重复发送。"
                onRead={() => void owner.restore()}
              />
            )}
            {view &&
              view.connection !== "attached" &&
              state.connection === "connected" && (
                <StatePanel
                  kind="readonly"
                  title="当前会话尚未恢复连接"
                  onRead={() => void owner.restore()}
                />
              )}
            {state.preferenceSave?.failed && (
              <StatePanel
                kind="error"
                title="对话选择尚未保存"
                onRead={() => owner.retryPreference()}
              />
            )}
          </div>
        </div>
        <div className="rss-assistant-input">
          {state.history.has(state.selected) && (
            <div className="rss-context-chip">
              <span>
                {owner.historyCurrent(state)
                  ? "下一条输入将带入已确认历史"
                  : "历史预览已变化，请重新预览或移除"}
              </span>
              <Button
                type="text"
                size="small"
                aria-label="移除待带入历史"
                icon={<CloseOutlined />}
                onClick={owner.clearHistory}
              />
            </div>
          )}
          {stream.newOutput && (
            <Button
              className="rss-latest-output"
              size="small"
              onClick={stream.latest}
            >
              有新内容 · 回到最新
            </Button>
          )}
          {context && (
            <div className="rss-context-chip">
              <span>当前附加：{context.path.join(" / ")}</span>
              <Button
                type="text"
                size="small"
                aria-label="移除页面上下文"
                icon={<CloseOutlined />}
                onClick={() => owner.removeContext()}
              />
            </div>
          )}
          {owner.compositionError(state) && (
            <p className="rss-muted">{owner.compositionError(state)}</p>
          )}
          <MessageComposer
            value={owner.draft.get(state)}
            onChange={owner.draft.set}
            onSend={() => void owner.prompt()}
            onCancel={cancelling ? () => void owner.cancel() : undefined}
            canSend={
              owner.canSend(state) &&
              owner.connectionReady(state) &&
              !owner.compositionError(state)
            }
            busy={
              cancelling || state.sending.has(state.selected) || state.opening
            }
          />
          {owner.busy(state) && (
            <div className="rss-assistant-compose-actions">
              <span className="rss-muted">
                当前回复仍在进行；停止请求与生成结束分别确认。
              </span>
              {owner.draft.get(state).trim() && (
                <>
                  <Button
                    size="small"
                    disabled={
                      !owner.canSend(state) || !!owner.compositionError(state)
                    }
                    onClick={() => void owner.prompt("queue_next")}
                  >
                    将输入排入下一轮
                  </Button>
                  {owner.canSteer(state) && (
                    <Button
                      size="small"
                      onClick={() => void owner.prompt("steer")}
                    >
                      转向当前回复
                    </Button>
                  )}
                </>
              )}
            </div>
          )}
        </div>
      </section>
    </div>
  );
}
