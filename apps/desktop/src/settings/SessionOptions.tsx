import { useEffect, useState } from "react";
import { Alert, Button, InputNumber, Select, Tag } from "antd";
import { useSnapshot } from "valtio";
import { DetailDrawer } from "@rss-mdm-agent/ui";
import type { Workspace } from "../state/workspace";
import { operationMessage } from "../assistant/controller";

function resumeDescription(reason: string) {
  switch (reason) {
    case "available":
      return "由原提供方核实并恢复同一模型上下文，不重发旧输入或设备操作。";
    case "retired":
      return "会话已退休，仅可阅读；请从二级菜单新建会话。";
    case "unknown":
      return "原提供方尚未确认恢复能力。";
    case "unsupported":
      return "原提供方不支持恢复上下文；可明确开始新上下文。";
    case "connection-required":
      return "原连接未就绪；请核对配置并选择一条已验证连接。";
    case "context-unavailable":
      return "该提供方不能跨进程恢复此上下文；请核对原记录后开始新上下文。";
    case "run-active":
      return "原输入仍在执行或派发，先读取原状态，不重新发送。";
    case "reconcile-first":
      return "原请求或会话操作回执未确认，先读取同一会话。";
    case "context-pending-recovery":
      return "已选择下次使用的连接；原上下文仍需恢复，请明确开始新上下文后再输入。";
    case "context-pending":
      return "已安排新连接或新上下文，下一条输入再开始；已接收的队列按原连接完成。";
    case "busy":
      return "正在处理原会话操作，请等待本次回执。";
    default:
      return "请先连接 AI Host 并读取原会话。";
  }
}

/** Controls consume the same assistant owner; this drawer owns only unsent selection UI. */
export function SessionOptions({
  workspace,
  onClose,
}: {
  workspace: Workspace;
  onClose(): void;
}) {
  const owner = workspace.assistant,
    state = useSnapshot(owner.state),
    view = owner.view(state),
    ready = state.connections.filter((row) => row.status === "ready"),
    current = owner.selectedConnectionId(state),
    [target, setTarget] = useState<string>(),
    [range, setRange] = useState("recent"),
    [recent, setRecent] = useState(5);
  useEffect(() => {
    setTarget(
      ready.some((row) => row.connectionId === current) ? current : undefined,
    );
  }, [current, state.selected, state.connections]);
  useEffect(() => () => owner.discardHistoryPreview(), [owner]);
  const pending = state.optionPending.has(state.selected),
    blocked =
      state.connection !== "connected" ||
      state.optionBusy ||
      state.opening ||
      pending ||
      state.pending.has(state.selected) ||
      state.sending.has(state.selected),
    preview = state.historyPreview,
    confirmed = state.history.get(state.selected),
    error = state.optionError || state.errors.get(state.selected),
    reason = owner.resumeReason(state);
  return (
    <DetailDrawer
      open
      title="会话选项"
      description="连接选择、模型上下文与下一条输入的历史范围"
      onClose={onClose}
      footer={<Button onClick={onClose}>完成</Button>}
    >
      <div className="rss-stack rss-session-options">
        <p className="rss-muted rss-reference">
          {state.selected
            ? `原会话：${state.selected}`
            : "尚未创建会话；第一次明确发送才创建。"}
        </p>
        {error && <Alert type="warning" title={operationMessage(error)} />}
        {pending && (
          <Alert
            type="warning"
            title="原会话操作回执尚未确认"
            description="先读取同一会话，不重复切换、恢复或发送新输入。"
          />
        )}
        <label className="rss-field">
          <span>本会话连接</span>
          <Select
            aria-label="本会话连接"
            placeholder="请选择已验证连接"
            value={target}
            disabled={blocked}
            options={ready.map((row) => ({
              value: row.connectionId,
              label: `${row.presentation.name} · ${row.provider} · 版本 ${row.configRevision}`,
            }))}
            onChange={(id) => {
              setTarget(id);
              owner.discardHistoryPreview();
            }}
          />
        </label>
        <Button
          disabled={blocked || !target || target === current}
          onClick={() => {
            if (target) void owner.chooseConnection(target);
          }}
        >
          应用会话连接
        </Button>
        <p className="rss-muted">
          只改变当前会话或这张空白草稿的选择。新会话默认连接在连接卡片单独设置；切换不发送输入，已接收的队列先按原连接完成。
        </p>
        {view?.connectionPending && (
          <Tag color="gold">下次输入使用新连接或新上下文</Tag>
        )}
        <Button
          disabled={
            blocked ||
            !view ||
            !owner.connectionReady(state) ||
            !target ||
            target !== current ||
            owner.busy(state) ||
            (!!view.connectionPending &&
              view.sessionStatus !== "recovery_required") ||
            view.sessionStatus === "retired"
          }
          onClick={() => {
            if (target) void owner.chooseConnection(target, true);
          }}
        >
          下次输入开始新上下文
        </Button>
        <section className="rss-stack" aria-label="历史带入选项">
          <h3>带入历史（默认不带入）</h3>
          <p className="rss-muted">
            先明确开始新上下文或切换连接，再选择带入历史。只预览已完成的用户和助手稳定文本，不带工具输出、系统指令或原始附件。确认后随下一条输入带入一次，不在预览时发送。
          </p>
          {confirmed && (
            <Alert
              type={owner.historyCurrent(state) ? "info" : "warning"}
              title={
                owner.historyCurrent(state)
                  ? "已确认下一条输入带入这些文本"
                  : "已确认预览已变化，发送前请重新预览或移除"
              }
            />
          )}
          <Select
            aria-label="历史预览范围"
            value={range}
            disabled={blocked}
            options={[
              { value: "recent", label: "最近 N 轮已完成文本" },
              { value: "all", label: "全部已完成文本（最多 64 KiB）" },
            ]}
            onChange={(value) => {
              setRange(value);
              owner.discardHistoryPreview();
            }}
          />
          {range === "recent" && (
            <InputNumber
              aria-label="历史轮数"
              min={1}
              max={10000}
              value={recent}
              disabled={blocked}
              onChange={(value) => {
                if (value != null) setRecent(value);
                owner.discardHistoryPreview();
              }}
            />
          )}
          <Button
            disabled={
              blocked ||
              !view ||
              !view.connectionPending ||
              !owner.connectionReady(state) ||
              target !== current ||
              owner.busy(state) ||
              view.sessionStatus !== "active"
            }
            onClick={() =>
              void owner.prepareHistory(range === "recent" ? recent : undefined)
            }
          >
            预览已完成文本
          </Button>
          {preview && (
            <>
              <pre className="rss-history-preview">
                {preview.text || "没有可带入的已完成文本"}
              </pre>
              <p className="rss-muted rss-reference">
                连接版本 {preview.configRevision} · 历史水位{" "}
                {preview.throughSequence} · {preview.commandIds.length} 个原命令
                · 摘要 {preview.contentHash}
              </p>
              <Button
                disabled={blocked || !preview.text || !owner.previewCurrent()}
                onClick={() => owner.confirmHistory()}
              >
                确认带入这些文本
              </Button>
            </>
          )}
          <Button onClick={owner.clearHistory}>不带入历史</Button>
        </section>
        <section className="rss-stack" aria-label="原模型上下文恢复">
          <h3>原模型上下文</h3>
          {view?.sessionStatus === "recovery_required" && (
            <Tag color="gold">原上下文需要恢复</Tag>
          )}
          <p>{resumeDescription(reason)}</p>
          <Button
            disabled={!owner.canResume(state)}
            onClick={() => void owner.restore(true)}
          >
            恢复原模型上下文
          </Button>
          <Button
            disabled={
              !state.selected ||
              state.connection !== "connected" ||
              state.opening ||
              state.optionBusy
            }
            onClick={() => void owner.restore()}
          >
            读取原会话状态
          </Button>
        </section>
      </div>
    </DetailDrawer>
  );
}
