import { useEffect, useRef, type ReactNode } from "react";
import {
  Alert,
  Button,
  Checkbox,
  Collapse,
  Input,
  InputNumber,
  Progress,
  Radio,
  Select,
  Steps,
  Switch,
  Tabs,
  Tag,
} from "antd";
import { useSnapshot } from "valtio";
import {
  getValue,
  isComponentNode,
  type ComponentNode,
} from "@a2ui/web_core/v0_9";
import type { OperationController } from "@rss-mdm-agent/ai-ui-bridge";
import type {
  OperationField,
  OperationEvent,
} from "@rss-mdm-agent/ai-contract/operations";

const text = (value: unknown) =>
  typeof value === "string"
    ? value.slice(0, 8192)
    : typeof value === "number" || typeof value === "boolean"
      ? String(value)
      : Array.isArray(value) && value.every((item) => typeof item === "string")
        ? value.join("、").slice(0, 8192)
        : "未提供";
const phaseLabels = {
  ready: "待核对",
  submitting: "已提交，等待回执",
  waiting: "等待处理",
  running: "执行中",
  execution_ended: "执行已结束，等待核实",
  failed: "已失败",
  unknown: "结果尚未确认",
  succeeded: "已核实符合预期",
  pending_effect: "等待实际效果核实",
  cancelled: "已确认取消",
  denied: "服务拒绝",
};
export function OperationFieldView({
  controller,
  field,
}: {
  controller: OperationController;
  field: OperationField;
}) {
  const state = useSnapshot(controller.state),
    input = useRef<HTMLInputElement>(null),
    value = state.draft.values[field.id],
    disabled = !controller.editable(field.id),
    options = field.options?.map((option) => ({
      value: option.id,
      label: option.disabledReason
        ? `${option.label}（${option.disabledReason}）`
        : option.label,
      disabled: !!option.disabledReason,
    }));
  useEffect(() => {
    const node = input.current;
    return () => {
      if (node) node.value = "";
    };
  }, []);
  let control: ReactNode;
  switch (field.component) {
    case "SecretField":
      control = (
        <>
          <input
            type="password"
            className="rss-ai-secret"
            ref={input}
            aria-label={field.label}
            autoComplete="off"
            defaultValue=""
            maxLength={field.maxLength ?? 8192}
            disabled={
              disabled ||
              !controller.secretAvailable ||
              state.capturing.has(field.id)
            }
            onBlur={(event) => {
              const value = event.currentTarget.value;
              event.currentTarget.value = "";
              if (value) void controller.secret(field.id, value);
            }}
          />
          {!controller.secretAvailable && (
            <span className="rss-muted">
              当前操作没有可用的受控秘密输入入口。
            </span>
          )}
          {state.secrets[field.id] && (
            <span>本次秘密输入已保管；不会加入消息或普通草稿。</span>
          )}
        </>
      );
      break;
    case "NumberField":
      control = (
        <InputNumber
          aria-label={field.label}
          value={typeof value === "number" ? value : null}
          min={field.min}
          max={field.max}
          disabled={disabled}
          onChange={(next) => controller.edit(field.id, next)}
        />
      );
      break;
    case "Switch":
      control = (
        <>
          <Switch
            aria-label={field.label}
            checked={value === true}
            disabled={disabled}
            onChange={(next) => controller.edit(field.id, next)}
          />
          {field.submitMode === "immediate" && (
            <span className="rss-muted">更改即提交本项设置。</span>
          )}
        </>
      );
      break;
    case "Checkbox":
      control = (
        <Checkbox
          aria-label={field.label}
          checked={value === true}
          disabled={disabled}
          onChange={(event) => controller.edit(field.id, event.target.checked)}
        >
          {field.label}
        </Checkbox>
      );
      break;
    case "SelectField":
      control = (
        <Select
          aria-label={field.label}
          value={typeof value === "string" ? value : undefined}
          options={options}
          disabled={disabled}
          onChange={(next) => controller.edit(field.id, next)}
        />
      );
      break;
    case "RadioGroup":
      control = (
        <Radio.Group
          aria-label={field.label}
          value={value}
          options={options}
          disabled={disabled}
          onChange={(event) => controller.edit(field.id, event.target.value)}
        />
      );
      break;
    case "PathPicker":
      control = (
        <div className="rss-ai-path">
          <Input
            aria-label={field.label}
            value={typeof value === "string" ? value : ""}
            maxLength={field.maxLength ?? 8192}
            disabled={disabled}
            onChange={(event) => controller.edit(field.id, event.target.value)}
          />
          <Button
            disabled={disabled || !controller.pathAvailable}
            onClick={() => void controller.choosePath(field.id)}
          >
            选择路径
          </Button>
        </div>
      );
      break;
    case "DateTimeField":
      control = (
        <Input
          aria-label={field.label}
          value={typeof value === "string" ? value : ""}
          placeholder="日期时间，包含时区"
          maxLength={field.maxLength ?? 8192}
          disabled={disabled}
          onChange={(event) => controller.edit(field.id, event.target.value)}
        />
      );
      break;
    default:
      control = (
        <Input
          aria-label={field.label}
          value={typeof value === "string" ? value : ""}
          maxLength={field.maxLength ?? 8192}
          disabled={disabled}
          onChange={(event) => controller.edit(field.id, event.target.value)}
        />
      );
  }
  return (
    <div className="rss-ai-field">
      <label>
        {field.label}
        {field.required ? " · 必填" : ""}
      </label>
      {control}
      {field.reason && <p className="rss-muted">{field.reason}</p>}
      {state.errors[field.id] && (
        <p role="alert" className="rss-ai-field-error">
          {state.errors[field.id]}
        </p>
      )}
    </div>
  );
}
export function OperationActions({
  controller,
}: {
  controller: OperationController;
}) {
  const state = useSnapshot(controller.state);
  const labels: Record<Exclude<OperationEvent["kind"], "answer">, string> = {
    submit: "确认本次具体动作",
    cancel: "请求取消",
    refresh: "查询原操作",
  };
  return (
    <div className="rss-ai-operation-actions">
      {state.snapshot.allowed
        .filter((action) => action !== "answer")
        .map((action) => (
          <Button
            key={action}
            type={action === "submit" ? "primary" : "default"}
            danger={action === "cancel"}
            disabled={!controller.can(action)}
            onClick={() => void controller.act(action)}
          >
            {labels[action as keyof typeof labels]}
          </Button>
        ))}
      {state.stale && !state.unknown && (
        <Button disabled={state.busy} onClick={() => controller.rebase()}>
          将草稿用于当前版本
        </Button>
      )}
    </div>
  );
}
export function OperationNode({
  node,
  props,
  controller,
  children,
  renderChild,
}: {
  node: ComponentNode;
  props: Record<string, unknown>;
  controller: OperationController;
  children: ReactNode;
  renderChild(value: unknown): ReactNode;
}) {
  const state = useSnapshot(controller.state),
    snapshot = state.snapshot,
    field = controller.presentation.fields.find(
      (field) => node.componentId === `field-${field.id}`,
    );
  if (field) {
    const step =
      controller.presentation.template === "GuidedFlow"
        ? controller.presentation.steps?.at(state.step)
        : undefined;
    return step && !step.fields.includes(field.id) ? null : (
      <OperationFieldView controller={controller} field={field} />
    );
  }
  switch (node.type) {
    case "OperationSurface":
      return <div className="rss-ai-operation-body">{children}</div>;
    case "Section":
      return (
        <section className="rss-ai-section">
          <h4>{typeof props.label === "string" ? props.label : "参数"}</h4>
          {children}
        </section>
      );
    case "FieldGroup":
      return <div className="rss-ai-fields">{children}</div>;
    case "Disclosure":
      return (
        <Collapse
          items={[
            {
              key: node.instanceId,
              label: typeof props.label === "string" ? props.label : "参数",
              children,
            },
          ]}
        />
      );
    case "Tabs": {
      const nodes = Array.isArray(props.children)
        ? props.children.filter(isComponentNode)
        : [];
      if (controller.presentation.template === "GuidedFlow")
        return <div>{nodes.map(renderChild)}</div>;
      return (
        <Tabs
          items={nodes.map((child) => ({
            key: child.instanceId,
            label: text(getValue(child.props).label),
            children: renderChild(child),
          }))}
        />
      );
    }
    case "Notice":
      return (
        <>
          {[state.notice, state.fallbackReason, snapshot.reason]
            .filter(Boolean)
            .map((notice, i) => (
              <Alert key={i} type="warning" title={notice} />
            ))}
        </>
      );
    case "ReadonlyRow":
      return (
        <dl className="rss-ai-facts">
          <dt>来源</dt>
          <dd>{snapshot.presentation.source}</dd>
          <dt>目标</dt>
          <dd>{snapshot.presentation.targetLabel}</dd>
          <dt>动作影响</dt>
          <dd>{snapshot.presentation.impact}</dd>
        </dl>
      );
    case "ApprovalStatus":
      return (
        <dl className="rss-ai-facts">
          <dt>本次用户确认</dt>
          <dd>{snapshot.approval.user}</dd>
          <dt>企业准入</dt>
          <dd>{snapshot.approval.enterprise}</dd>
          <dt>系统权限</dt>
          <dd>{snapshot.approval.elevation}</dd>
        </dl>
      );
    case "ObjectList":
    case "SelectableTable":
      return (
        <div className="rss-ai-objects">
          {snapshot.objects.map((object) => (
            <button
              type="button"
              key={object.id}
              aria-pressed={state.draft.selection.includes(object.id)}
              disabled={!controller.can("submit") || !!object.disabledReason}
              onClick={() => controller.select(object.id)}
            >
              <strong>{object.label}</strong>
              <span>{object.detail}</span>
              {object.disabledReason && <small>{object.disabledReason}</small>}
            </button>
          ))}
        </div>
      );
    case "ChangePreview":
      return Object.keys(snapshot.proposal).length ? (
        <div className="rss-ai-change-preview">
          <h4>变更核对</h4>
          {controller.presentation.fields
            .filter(
              (field) =>
                field.component !== "SecretField" &&
                !snapshot.secretFields?.includes(field.id) &&
                field.id in state.draft.values,
            )
            .map((field) => (
              <div key={field.id}>
                <strong>{field.label}</strong>
                <span>{text(snapshot.current[field.id])}</span>
                <span aria-hidden="true">→</span>
                <span>{text(state.draft.values[field.id])}</span>
              </div>
            ))}
        </div>
      ) : null;
    case "ConfirmationPanel":
      return (
        <p className="rss-muted">
          只确认当前展示的具体动作；企业准入与系统权限仍由原执行服务检查。
        </p>
      );
    case "PendingInteraction":
      return snapshot.interaction ? (
        <section className="rss-ai-section">
          <h4>{snapshot.interaction.question}</h4>
          {snapshot.interaction.options.map((option) => (
            <Button
              key={option.id}
              disabled={!controller.can("answer")}
              onClick={() => void controller.act("answer", option.id)}
            >
              {option.label}
            </Button>
          ))}
        </section>
      ) : null;
    case "Progress":
      return snapshot.progress && snapshot.progress.total > 0 ? (
        <div>
          <Progress
            percent={Math.min(
              100,
              Math.floor(
                (snapshot.progress.completed / snapshot.progress.total) * 100,
              ),
            )}
            showInfo={false}
          />
          <span>
            {snapshot.progress.completed} / {snapshot.progress.total}
          </span>
        </div>
      ) : null;
    case "ResultSummary":
      return (
        <div className="rss-ai-result">
          <Tag
            color={
              snapshot.phase === "succeeded"
                ? "green"
                : ["failed", "denied"].includes(snapshot.phase)
                  ? "red"
                  : "gold"
            }
          >
            {phaseLabels[snapshot.phase]}
          </Tag>
          {snapshot.result && <p>{snapshot.result}</p>}
        </div>
      );
    case "LogView":
      return snapshot.logs.length ? (
        <details>
          <summary>读取执行记录</summary>
          <pre>{snapshot.logs.join("\n")}</pre>
        </details>
      ) : null;
    case "StepFlow":
      return (
        <div>
          <Steps
            size="small"
            current={state.step}
            items={controller.presentation.steps?.map((step) => ({
              title: step.label,
            }))}
          />
          <div className="rss-ai-operation-actions">
            <Button
              disabled={state.step === 0 || state.busy}
              onClick={() => controller.previous()}
            >
              上一步
            </Button>
            <Button
              disabled={
                state.busy ||
                state.step >= (controller.presentation.steps?.length ?? 1) - 1
              }
              onClick={() => controller.next()}
            >
              下一步
            </Button>
          </div>
        </div>
      );
    case "ActionBar":
      return <OperationActions controller={controller} />;
    default:
      throw Error("unsupported trusted operation component");
  }
}
