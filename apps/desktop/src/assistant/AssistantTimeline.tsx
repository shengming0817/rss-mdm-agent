import { useSnapshot } from "valtio";
import { Button, Tag } from "antd";
import { MessageEntry } from "@rss-mdm-agent/ui";
import { RuntimeSurface } from "@rss-mdm-agent/ai-ui-bridge";
import type { Workspace } from "../state/workspace";
import { boundedText } from "../software/presentation";
import { copyText } from "./clipboard";
import { cancelNote } from "./command-presentation";
import { QuestionCard, QuestionFacts, PermissionCards } from "./Questions";
import { mountQuestionNode } from "./operation-ui/nodes";
import { OperationCard } from "./operation-ui/OperationCard";

/** Preserve the client's source order. Tool text is never substituted for service-owned action facts. */
export function AssistantTimeline({
  workspace,
  active,
}: {
  workspace: Workspace;
  active: boolean;
}) {
  const owner = workspace.assistant,
    state = useSnapshot(owner.state),
    view = owner.view(state),
    runtime = owner.runtime.current;
  if (!view) return null;
  const live =
    state.connection === "connected" &&
    view.connection === "attached" &&
    view.sessionStatus === "active";
  return (
    <div
      className="rss-messages rss-assistant-timeline"
      data-busy={owner.busy(state) ? "true" : "false"}
      role="log"
      aria-label="对话消息"
    >
      {view.timeline.map((item) => {
        const key = JSON.stringify([
          view.namespace,
          view.generation,
          item.kind,
          item.key,
        ]);
        if (item.kind === "prompt") {
          const command = view.commands[item.key]?.command;
          return command?.input.type === "prompt" ? (
            <MessageEntry
              key={key}
              message={{ id: item.key, kind: "user", text: command.input.text }}
              onCopy={copyText}
            />
          ) : null;
        }
        if (item.kind === "message") {
          const message = view.messages[item.key];
          return message ? (
            <MessageEntry
              key={key}
              message={{
                id: item.key,
                kind: "assistant",
                text: message.text,
                stable: message.stable,
              }}
              onCopy={copyText}
            />
          ) : null;
        }
        if (item.kind === "interaction") {
          return Object.values(view.surfaces).some(
            (surface) => surface.interactionId === item.key,
          ) ? null : (
            <QuestionCard key={key} workspace={workspace} id={item.key} />
          );
        }
        if (item.kind === "surface") {
          const surface = view.surfaces[item.key],
            interaction = surface && view.interactions[surface.interactionId];
          if (!surface || !interaction) return null;
          return (
            <section
              key={key}
              className="rss-question-card"
              aria-label="本次互动问题"
            >
              <QuestionFacts interaction={interaction} />
              {runtime && live ? (
                <RuntimeSurface
                  runtime={runtime}
                  sessionId={view.namespace.sessionId}
                  instanceId={item.key}
                  drafts={owner.surfaceDrafts}
                  mount={mountQuestionNode}
                  fallback={<QuestionFacts interaction={interaction} />}
                />
              ) : (
                <p className="rss-muted">当前只能读取，恢复原会话后再核对。</p>
              )}
              <Button
                type="link"
                size="small"
                onClick={() => void owner.restore()}
              >
                读取原问题状态
              </Button>
            </section>
          );
        }
        if (item.kind === "tool") {
          const tool = view.tools[item.key];
          return tool ? (
            <details key={key} className="rss-tool-record">
              <summary>
                {boundedText(tool.name, 160)} ·{" "}
                {tool.status === "pending"
                  ? "工具处理中"
                  : tool.status === "failed"
                    ? "工具回执失败"
                    : "工具结果已返回"}
              </summary>
              {tool.result?.text && (
                <p>{boundedText(tool.result.text, 8192)}</p>
              )}
              <p className="rss-muted">
                工具文本不证明设备操作已经成功；具体动作以原服务事实为准。
              </p>
            </details>
          ) : null;
        }
        if (item.kind === "delivery" && view.deliveries[item.key])
          return (
            <OperationCard
              key={key}
              workspace={workspace}
              operationId={item.key}
              generation={view.generation}
              active={active}
              live={live}
            />
          );
        return null;
      })}
      <PermissionCards workspace={workspace} />
      <div className="rss-ai-run-notes" aria-label="当前回复状态">
        {Object.values(view.commands).flatMap((command) => {
          const note = cancelNote(command);
          return note ? [<p key={command.command.commandId}>{note}</p>] : [];
        })}
        {owner.busy(state) && (
          <Tag>{owner.canCancel(state) ? "回复进行中" : "原消息仍在处理"}</Tag>
        )}
      </div>
    </div>
  );
}
