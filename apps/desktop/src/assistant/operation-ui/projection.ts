import type {
  TaskDetail,
  BackendRequest,
} from "@rss-mdm-agent/execution-bindings/tasks";
import type {
  OperationField,
  OperationPresentation,
  OperationSnapshot,
  OperationValue,
} from "@rss-mdm-agent/ai-contract/operations";
import { taskPresentation } from "../execution-presentation";

type Policy = Extract<BackendRequest["action"], { kind: "policy" }>;
export type FieldBinding = {
  id: string;
  key: string;
  editable: boolean;
  value: unknown;
};
const plain = (value: unknown): value is Record<string, any> =>
  !!value && typeof value === "object" && !Array.isArray(value);
const value = (input: unknown): OperationValue => {
  if (["string", "number", "boolean"].includes(typeof input))
    return input as OperationValue;
  return JSON.stringify(input) ?? "";
};
function unsafeSchema(input: unknown): boolean {
  if (Array.isArray(input)) return input.some(unsafeSchema);
  if (!plain(input)) return false;
  return (
    input.writeOnly === true ||
    input.format === "password" ||
    Object.values(input).some(unsafeSchema)
  );
}
const presentation = (
  template: OperationPresentation["template"],
  fields: OperationField[] = [],
): OperationPresentation => ({
  version: 1,
  template,
  fields,
  rules: [],
  sections: [],
});
/** Only the finite component vocabulary is compiled. The service validates the full original schema. */
function policyFields(action: Policy, editable: boolean) {
  const fields: OperationField[] = [],
    bindings: FieldBinding[] = [];
  let specialized = editable,
    blocked = false;
  const task = action.task;
  if (task.kind !== "script")
    return { fields, bindings, specialized: false, blocked };
  const schema = plain(task.parameterSchema) ? task.parameterSchema : {};
  const unsafe = unsafeSchema(schema);
  const properties = plain(schema.properties) ? schema.properties : {};
  const required = Array.isArray(schema.required) ? schema.required : [];
  for (const [key, source] of Object.entries(task.parameterSources)) {
    const spec = plain(properties[key]) ? properties[key] : {};
    const original =
      source.kind === "fixed" ? source.value : action.choice.parameters[key];
    const id = `field${bindings.length}`;
    const f: OperationField = {
      id,
      label: key,
      component: "TextField",
      required: required.includes(key),
    };
    // No secret material can enter generic current/proposal/history or an untrusted capture port.
    if (unsafe) {
      f.readonly = true;
      f.reason = "此操作尚无可信秘密输入接缝";
      bindings.push({ id, key, editable: false, value: undefined });
      fields.push(f);
      blocked = true;
      specialized = false;
      continue;
    }
    const scalar = ["string", "number", "integer", "boolean"].includes(
      spec.type,
    );
    const options =
      Array.isArray(spec.enum) &&
      spec.enum.every(
        (v: unknown) =>
          typeof v === "string" && /^[a-zA-Z0-9_-]{1,128}$/.test(v),
      );
    if (!scalar || (spec.enum && !options)) specialized = false;
    if (options) {
      f.component = "SelectField";
      f.options = spec.enum.map((id: string) => ({ id, label: id }));
    } else if (spec.type === "boolean") {
      f.component = "Switch";
      f.submitMode = "draft";
    } else if (["number", "integer"].includes(spec.type)) {
      f.component = "NumberField";
      if (typeof spec.minimum === "number") f.min = spec.minimum;
      if (typeof spec.maximum === "number") f.max = spec.maximum;
    }
    if (spec.type === "string")
      f.maxLength = Math.min(
        8192,
        typeof spec.maxLength === "number" ? spec.maxLength : 8192,
      );
    const canEdit =
      editable && source.kind === "input" && scalar && (!spec.enum || options);
    if (!canEdit) {
      f.readonly = true;
      f.reason =
        source.kind === "fixed" ? "由可信策略固定" : "最终参数只读审阅";
    }
    if (!scalar && original !== undefined) f.component = "TextField";
    fields.push(f);
    bindings.push({ id, key, editable: canEdit, value: original });
  }
  return { fields, bindings, specialized, blocked };
}
export function requestId(view: TaskDetail) {
  if (view.reference.kind !== "request") throw Error("操作不是原请求");
  return view.reference.request;
}
export function operationOrigin(view: TaskDetail) {
  const facts = view.facts;
  if (facts.kind !== "execution" && facts.kind !== "preparation")
    throw Error("操作来源不适用");
  const origin =
    facts.kind === "execution"
      ? facts.value.action.initiator
      : facts.value.trigger;
  return origin.kind === "backend" ? origin.trigger : origin;
}
export function operationProjection(
  view: TaskDetail,
  scope: OperationSnapshot["scope"],
  revision: number,
  now = Date.now(),
  preparation?: BackendRequest,
) {
  const snapshot: OperationSnapshot = {
    scope,
    revision,
    baseline: scope.instance,
    presentation: {
      title: "设备操作",
      source: "认证执行服务",
      targetLabel: scope.target,
      impact: "执行仍需服务核验授权与系统条件",
    },

    current: {},
    proposal: {},
    phase: "unknown",
    allowed: ["refresh"],
    objects: [],
    logs: [],
    approval: {
      user: "未取得确认事实",
      enterprise: "由执行服务核验",
      elevation: "由执行身份和系统条件决定",
    },
  };
  let ui = presentation("TaskExecution"),
    bindings: FieldBinding[] = [];
  if (view.facts.kind === "execution") {
    const details = view.facts.value,
      status = details.status,
      display = taskPresentation(details);
    snapshot.presentation.title = details.action.operation.resource.id;
    snapshot.presentation.source = `${status.mode === "test" ? "测试执行器，无真实设备效果" : "生产执行服务"} · ${details.action.actor}`;
    snapshot.result = `${display.label}。${display.stage}。${display.cancel}`;
    snapshot.phase =
      status.software === "restartPending"
        ? "pending_effect"
        : status.phase === "verified"
          ? status.assessment === "satisfied"
            ? "succeeded"
            : status.assessment === "unknown" || !status.assessment
              ? "unknown"
              : "failed"
          : status.phase === "running"
            ? "running"
            : status.phase === "executionEnded"
              ? "execution_ended"
              : status.phase === "outcomeUnknown"
                ? "unknown"
                : status.phase === "cancelled"
                  ? "cancelled"
                  : status.phase === "admissionDenied"
                    ? "denied"
                    : status.phase === "failedBeforeDispatch"
                      ? "failed"
                      : "waiting";
    snapshot.approval.enterprise = "准入状态见可信执行详情";
    snapshot.approval.user =
      status.phase === "confirmationRequired"
        ? "执行服务要求确认，请前往任务处理"
        : "以原请求确认记录为准";
    snapshot.approval.elevation =
      details.action.runAs.kind === "system"
        ? "系统执行身份；不表示用户授予提权"
        : "本机用户执行身份";
    if (view.allowedActions.includes("cancel")) snapshot.allowed.push("cancel");
  } else if (view.facts.kind === "preparation") {
    const record = preparation ?? view.facts.value,
      action = record.action;
    const editable = record.state === "draft";
    ui = presentation("OperationApproval");
    snapshot.phase = ["draft", "awaitingConfirmation"].includes(record.state)
      ? "ready"
      : record.state === "failed"
        ? "failed"
        : record.state === "cancelled"
          ? "cancelled"
          : record.state === "observed"
            ? "execution_ended"
            : "waiting";
    snapshot.expiresAtMs =
      action.kind === "offered"
        ? action.offer.expiresAt * 1000
        : action.expiresAtUnixMs;
    const expired = now >= snapshot.expiresAtMs;
    if (record.failure) snapshot.reason = `执行服务拒绝：${record.failure}`;
    else if (expired)
      snapshot.reason = "原准备已过期；草稿保留，不能沿用旧确认";
    else delete snapshot.reason;
    snapshot.approval.user = record.confirmation
      ? "已确认原请求最终参数"
      : editable
        ? "保存当前参数即表达本次确认"
        : record.state === "awaitingConfirmation"
          ? "等待本人确认"
          : "可信策略未要求确认";
    if (view.allowedActions.includes("cancel")) snapshot.allowed.push("cancel");
    if (
      !expired &&
      ((record.state === "draft" && !!preparation) ||
        (record.state === "awaitingConfirmation" && view.humanConfirmation))
    )
      snapshot.allowed.push("submit");
    if (action.kind === "policy") {
      const form = policyFields(action, editable);
      bindings = form.bindings;
      ui = presentation(
        form.specialized ? "SettingsForm" : "OperationApproval",
        form.fields,
      );
      snapshot.presentation.title = presentationText(
        action.presentation.displayName,
        `策略 ${action.choice.policyId}`,
      );
      snapshot.presentation.source = `登记策略 ${action.choice.policyId}`;
      snapshot.presentation.impact = presentationText(
        action.presentation.sideEffects,
      );
      snapshot.reason ??= "当前系统设置未由此策略读取；建议不是当前状态。";
      snapshot.approval.elevation =
        action.task.kind !== "enrollment" && action.task.runAs === "system"
          ? "系统执行身份；OS 条件由服务复核"
          : "本机登录用户执行";
      const task = action.task;
      snapshot.presentation.targetLabel =
        task.kind === "software" && task.target.kind === "user"
          ? `${scope.target} · 目标 OS 用户 ${task.target.identity} / 登录会话 ${task.target.sessionId}`
          : `${scope.target} · 本机设备`;
      snapshot.approval.enterprise =
        action.access.kind === "device" ? "设备准入" : "企业用户准入";
      for (const binding of bindings)
        if (binding.value !== undefined)
          snapshot.proposal[binding.id] = value(binding.value);
      snapshot.logs = [
        action.task.kind === "enrollment"
          ? `原生 MDM 注册：${action.task.organization}`
          : `资源：${action.task.resource.id} / ${action.task.resource.version}`,
        `前置条件：${presentationText(action.presentation.prerequisites)}`,
      ];
      if (action.runId) {
        snapshot.presentation.source = `已受理策略 ${action.choice.policyId}`;
        snapshot.approval.elevation = "实际执行身份以已受理任务及执行详情为准";
        snapshot.presentation.targetLabel = scope.target;
        snapshot.logs = ["提交前配置快照：", ...snapshot.logs];
        if (action.offer) {
          snapshot.presentation.title = action.offer.presentation.title;
          snapshot.logs.unshift(
            `已受理执行摘要：${JSON.stringify(action.offer.presentation.summary)}`,
          );
        }
      }
      if (form.blocked) {
        snapshot.allowed = snapshot.allowed.filter((a) => a !== "submit");
        snapshot.reason = "秘密输入接缝不可用，禁止提交。";
      }
    } else if (action.kind === "offered") {
      snapshot.presentation.title = action.offer.presentation.title;
      snapshot.logs = [JSON.stringify(action.offer.presentation.summary)];
      snapshot.presentation.source = `签名任务 ${action.offer.task} / ${action.offer.attempt}`;
    } else {
      snapshot.presentation.title = "可信软件状态观察";
      snapshot.result = action.observation
        ? JSON.stringify(action.observation)
        : "尚无可信观察结果";
      snapshot.allowed = ["refresh"];
    }
  }
  const origin = operationOrigin(view);
  if (origin.kind === "ai") {
    snapshot.presentation.source = `AI 发起 · ${snapshot.presentation.source}`;
    snapshot.logs.push(
      `AI 会话 / 调用：${origin.conversation} / ${origin.toolCall}`,
      `AI 配置：${origin.config.id} / ${origin.config.revision}`,
      `本机账号 / 会话：${origin.osSession.account.subject} / ${origin.osSession.session}`,
      `授权范围：${scope.authority} / ${scope.tenant} / ${scope.principal}`,
    );
  }
  return { snapshot, presentation: ui, bindings };
}

function presentationText(value: unknown, fallback = "未提供"): string {
  return typeof value === "string" && value.trim() ? value.trim() : fallback;
}
