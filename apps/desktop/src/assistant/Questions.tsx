import { useState, useSyncExternalStore } from "react";
import { Alert, Button, Input, Tag } from "antd";
import { useSnapshot } from "valtio";
import { questions } from "@rss-mdm-agent/ai-ui-bridge";
import type { InteractionView } from "@rss-mdm-agent/ai-client";
import type { Snapshot } from "valtio/vanilla";
import { StatePanel } from "@rss-mdm-agent/ui";
import type { Workspace } from "../state/workspace";
import { boundedText } from "../software/presentation";

export function QuestionFacts({
  interaction,
}: {
  interaction: Snapshot<InteractionView>;
}) {
  const parsed = questions(interaction.request);
  return (
    <div className="rss-question-facts">
      {parsed ? (
        parsed.map((question) => (
          <p key={question.question}>
            <strong>{question.header}</strong> {question.question}
          </p>
        ))
      ) : (
        <p>
          {boundedText(
            interaction.request.question ||
              interaction.request.prompt ||
              interaction.request.title,
            8192,
          ) || "原结构化问题暂时无法以互动表单显示。"}
        </p>
      )}
      <span className="rss-muted">
        原问题状态：
        {interaction.status === "answered"
          ? "已回答"
          : interaction.status === "pending"
            ? "等待回答"
            : interaction.status === "expired"
              ? "已过期"
              : "不可用"}
      </span>
    </div>
  );
}
/** Ordinary answer text carries no approval semantics; unsupported formats remain read-only. */
export function QuestionCard({
  workspace,
  id,
}: {
  workspace: Workspace;
  id: string;
}) {
  const owner = workspace.assistant,
    state = useSnapshot(owner.state),
    view = owner.view(state),
    interaction = view?.interactions[id],
    parsed = interaction ? questions(interaction.request) : undefined;
  if (!interaction || !view) return null;
  if (!parsed)
    return (
      <section className="rss-question-card">
        <QuestionFacts interaction={interaction} />
        <StatePanel
          kind="readonly"
          title="此问题当前只能读取"
          description="保留原会话和文本，重新读取后再核对可用的回答方式。"
          onRead={() => void owner.restore()}
        />
      </section>
    );
  return (
    <QuestionForm
      key={JSON.stringify([view.namespace, id, interaction.generation, parsed])}
      workspace={workspace}
      id={id}
    />
  );
}
function QuestionForm({ workspace, id }: { workspace: Workspace; id: string }) {
  const owner = workspace.assistant,
    state = useSnapshot(owner.state),
    view = owner.view(state)!,
    interaction = view.interactions[id],
    parsed = questions(interaction.request)!,
    key = JSON.stringify([
      "question",
      view.namespace.authorityId,
      view.namespace.tenantId,
      view.namespace.principalId,
      view.namespace.sessionId,
      id,
      interaction.generation,
      parsed,
    ]),
    memory = owner.surfaceDrafts.form(key),
    [error, setError] = useState("");
  useSyncExternalStore(memory.subscribe, memory.snapshot);
  const enabled = owner.answerable(id) && !memory.retired;
  const choose = (question: string, label: string, multiple: boolean) => {
    if (!owner.answerable(id) || memory.retired) return;
    const previous = memory.fields.get(question) ?? "",
      next = multiple && previous ? `${previous}\n${label}` : label;
    if (next.length > 8192) {
      setError("答案过长，请编辑后再选择提示。");
      return;
    }
    setError("");
    memory.edit(question, next);
  };
  return (
    <section className="rss-question-card" aria-label="本次结构化问题">
      <header>
        <span className="rss-ai-card-eyebrow">需要你的回答</span>
        <Tag>
          {enabled
            ? "等待回答"
            : interaction.status === "answered"
              ? "已回答"
              : "当前只读"}
        </Tag>
      </header>
      {parsed.map((question) => (
        <div className="rss-ai-field" key={question.question}>
          <label>
            {question.header} · {question.question}
          </label>
          <div className="rss-question-options">
            {question.options.map((option) => (
              <Button
                key={option.label}
                disabled={!enabled}
                title={option.description}
                onClick={() =>
                  choose(question.question, option.label, question.multiSelect)
                }
              >
                {option.label}
              </Button>
            ))}
          </div>
          <Input.TextArea
            aria-label={question.question}
            value={memory.fields.get(question.question) ?? ""}
            disabled={!enabled}
            maxLength={8192}
            autoSize={{ minRows: 2, maxRows: 5 }}
            onChange={(event) => {
              if (owner.answerable(id) && !memory.retired)
                memory.edit(question.question, event.target.value);
            }}
          />
          <p className="rss-muted">
            {question.multiSelect
              ? "选项按行填入，可编辑最终答案；提交的是每题一段文字。"
              : "可选择提示填入，也可以编辑最终答案。"}
          </p>
        </div>
      ))}
      {error && <Alert type="warning" title={error} />}
      <p className="rss-muted">
        问题有效至 {new Date(interaction.expiresAtMs).toLocaleString()}
        ；此回答不授予设备执行权限。
      </p>
      <Button
        type="primary"
        disabled={
          !enabled ||
          parsed.some(
            (question) => !(memory.fields.get(question.question) ?? "").trim(),
          )
        }
        onClick={() => {
          if (owner.answerable(id) && !memory.retired)
            void owner.respond(
              id,
              Object.fromEntries(
                parsed.map((question) => [
                  question.question,
                  memory.fields.get(question.question) ?? "",
                ]),
              ),
            );
        }}
      >
        提交本次回答
      </Button>
      {!enabled && <QuestionFacts interaction={interaction} />}
    </section>
  );
}
export function PermissionCards({ workspace }: { workspace: Workspace }) {
  const owner = workspace.assistant,
    state = useSnapshot(owner.state);
  return (
    <>
      {[...state.permissions.values()]
        .filter((item) => item.request.sessionId === state.selected)
        .map((item) => (
          <section
            key={item.id}
            className="rss-permission-card"
            aria-label="本次工具请求"
          >
            <span className="rss-ai-card-eyebrow">当前会话的工具请求</span>
            <h3>
              {boundedText(item.request.toolCall.title, 300) || "核对本次工具"}
            </h3>
            <p>
              允许一次只对应当前工具请求；企业准入和系统权限仍由原执行服务核验。
            </p>
            <div className="rss-ai-operation-actions">
              {item.request.options
                .filter(
                  (option) =>
                    option.kind === "allow_once" ||
                    option.kind === "reject_once",
                )
                .map((option) => (
                  <Button
                    key={option.optionId}
                    type={option.kind === "allow_once" ? "primary" : "default"}
                    onClick={() => owner.permission(item.id, option.optionId)}
                  >
                    {option.kind === "allow_once"
                      ? "仅允许本次请求"
                      : "拒绝本次请求"}
                  </Button>
                ))}
              <Button onClick={() => owner.permission(item.id)}>
                关闭并拒绝
              </Button>
            </div>
          </section>
        ))}
    </>
  );
}
