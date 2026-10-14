import type {
  BackendTaskView,
  BackendRequest,
} from "@rss-mdm-agent/execution-bindings/task-details";
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
  if (input === null || ["string", "number", "boolean"].includes(typeof input))
    return input as OperationValue;
  if (Array.isArray(input) && input.every((item) => typeof item === "string"))
    return input;
  return JSON.stringify(input) ?? "";
};
const presentation = (
  template: OperationPresentation["template"],
  fields: OperationField[] = [],
): OperationPresentation => ({
  version: 2,
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
  const task = action.directory.task;
  if (task.kind !== "script")
    return { fields, bindings, specialized: false, blocked };
  const schema = plain(task.parameterSchema) ? task.parameterSchema : {};
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
    if (spec.writeOnly === true || spec.format === "password") {
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
export function requestId(view: BackendTaskView) {
  if (view.kind === "execution") return view.value.status.operationRequestId;
  return view.value.action.kind === "offered"
    ? view.value.action.offer.request
    : view.value.action.choice.request;
}
export function operationOrigin(view: BackendTaskView) {
  const origin =
    view.kind === "execution"
      ? view.value.action.initiator
      : view.value.trigger;
  return origin.kind === "backend" ? origin.trigger : origin;
}
export function operationProjection(
  view: BackendTaskView,
  scope: OperationSnapshot["scope"],
  revision: number,
  now = Date.now(),
) {
  const snapshot: OperationSnapshot = {
    scope,
    revision,
    baseline: scope.instance,
    title: "设备操作",
    source: "认证执行服务",
    targetLabel: scope.target,
    impact: "执行仍需服务核验授权与系统条件",
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
  if (view.kind === "execution") {
    const details = view.value,
      status = details.status,
      display = taskPresentation(details);
    snapshot.title = details.action.operation.resource.id;
    snapshot.source = `${status.mode === "test" ? "测试执行器，无真实设备效果" : "生产执行服务"} · ${details.action.actor}`;
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
    snapshot.approval.enterprise =
      status.phase === "approvalRequired"
        ? "等待企业批准"
        : "准入状态见可信执行详情";
    snapshot.approval.user =
      status.phase === "confirmationRequired"
        ? "执行服务要求确认，请前往任务处理"
        : "以原请求确认记录为准";
    snapshot.approval.elevation =
      details.action.runAs.kind === "system"
        ? "系统执行身份；不表示用户授予提权"
        : "本机用户执行身份";
    if (
      !["verified", "cancelled", "failedBeforeDispatch"].includes(
        status.phase,
      ) &&
      !status.cancelRequested
    )
      snapshot.allowed.push("cancel");
  } else {
    const record = view.value,
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
    snapshot.reason = record.failure
      ? `执行服务拒绝：${record.failure}`
      : expired
        ? "原准备已过期；草稿保留，不能沿用旧确认"
        : undefined;
    snapshot.approval.user = record.confirmation
      ? "已确认原请求最终参数"
      : editable
        ? "保存当前参数即表达本次确认"
        : record.state === "awaitingConfirmation"
          ? "等待本人确认"
          : "可信策略未要求确认";
    if (!["failed", "cancelled", "observed"].includes(record.state))
      snapshot.allowed.push("cancel");
    if (!expired && ["draft", "awaitingConfirmation"].includes(record.state))
      snapshot.allowed.push("submit");
    if (action.kind === "policy") {
      const form = policyFields(action, editable);
      bindings = form.bindings;
      ui = presentation(
        form.specialized ? "SettingsForm" : "OperationApproval",
        form.fields,
      );
      snapshot.title = action.directory.selfService.displayName;
      snapshot.source = `登记策略 ${action.choice.policyId} / ${action.choice.policyRevision}`;
      snapshot.impact = action.directory.selfService.sideEffects;
      snapshot.reason ??= "当前系统设置未由此策略读取；建议不是当前状态。";
      snapshot.approval.elevation =
        action.directory.task.runAs === "system"
          ? "系统执行身份；OS 条件由服务复核"
          : "本机登录用户执行";
      snapshot.targetLabel = `${scope.target} · ${action.directory.subject.kind === "device" ? "设备目标" : `企业用户 ${action.directory.subject.user.principalId}`}`;
      for (const binding of bindings)
        if (binding.value !== undefined)
          snapshot.proposal[binding.id] = value(binding.value);
      snapshot.logs = [
        `执行版本：${action.choice.policyVersion}`,
        `资源：${action.directory.task.resource.id} / ${action.directory.task.resource.version}`,
        `前置条件：${action.directory.selfService.prerequisites}`,
      ];
      if (form.blocked) {
        snapshot.allowed = snapshot.allowed.filter((a) => a !== "submit");
        snapshot.reason = "秘密输入接缝不可用，禁止提交。";
      }
    } else if (action.kind === "offered") {
      snapshot.title = action.offer.title;
      snapshot.logs = [JSON.stringify(action.offer.summary)];
      snapshot.source = `签名任务 ${action.offer.task} / ${action.offer.attempt}`;
    } else {
      snapshot.title = "可信软件状态观察";
      snapshot.result = action.observation
        ? JSON.stringify(action.observation)
        : "尚无可信观察结果";
      snapshot.allowed = ["refresh"];
    }
  }
  return { snapshot, presentation: ui, bindings };
}
