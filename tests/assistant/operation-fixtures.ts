import type {
  OperationEvent,
  OperationHostPort,
  OperationPresentation,
  OperationSnapshot,
} from "../../packages/ai-contract/dist/operations.js";
export const operationTemplates: OperationPresentation["template"][] = [
  "SettingsForm",
  "ObjectActions",
  "ResourceSelection",
  "ChangeReview",
  "TaskExecution",
  "GuidedFlow",
  "OperationApproval",
];
export function operationFixture(
  template: OperationPresentation["template"] = "SettingsForm",
) {
  const presentation: OperationPresentation = {
    version: 2,
    template,
    layout: template === "ChangeReview" ? "disclosure" : "tabs",
    selection: template === "ObjectActions" ? "single" : "multiple",
    sections: [
      { id: "general", label: "常规" },
      { id: "advanced", label: "高级" },
    ],
    fields: [
      {
        id: "name",
        label: "名称",
        component: "TextField",
        required: true,
        maxLength: 64,
        section: "general",
      },
      {
        id: "enabled",
        label: "启用代理",
        component: "Switch",
        submitMode: "draft",
        section: "general",
      },
      {
        id: "address",
        label: "代理地址",
        component: "TextField",
        section: "general",
      },
      {
        id: "port",
        label: "代理端口",
        component: "NumberField",
        min: 1,
        max: 65535,
        section: "general",
      },
      {
        id: "network",
        label: "网络范围",
        component: "SelectField",
        options: [
          { id: "user", label: "当前用户" },
          { id: "device", label: "整台设备", disabledReason: "组织限制" },
        ],
        section: "general",
      },
      {
        id: "confirm",
        label: "启用通知",
        component: "Checkbox",
        section: "advanced",
      },
      {
        id: "power",
        label: "供电条件",
        component: "RadioGroup",
        options: [
          { id: "ac", label: "接通电源" },
          { id: "battery", label: "使用电池" },
        ],
        section: "advanced",
      },
      {
        id: "path",
        label: "配置路径",
        component: "PathPicker",
        section: "advanced",
      },
      {
        id: "time",
        label: "执行时间",
        component: "DateTimeField",
        section: "advanced",
      },
      {
        id: "secret",
        label: "操作密码",
        component: "SecretField",
        section: "advanced",
      },
      {
        id: "organization",
        label: "组织",
        component: "TextField",
        readonly: true,
        reason: "由组织管理",
        section: "advanced",
      },
    ],
    rules: [
      {
        field: "address",
        when: "enabled",
        equals: true,
        kind: "required",
        message: "启用代理时必须填写地址",
      },
    ],
    steps: [
      {
        id: "edit",
        label: "编辑参数",
        fields: ["name", "enabled", "address", "port", "network"],
      },
      {
        id: "review",
        label: "核对修改",
        fields: ["confirm", "power", "path", "time", "secret", "organization"],
      },
    ],
  };
  const snapshot: OperationSnapshot = {
    scope: {
      authority: "fixture",
      tenant: "test-tenant",
      principal: "test-user",
      session: "fixture-session",
      target: "fixture-target",
      instance: `fixture-${template}`,
    },
    revision: 1,
    baseline: "1",
    title: `${template} 操作示例`,
    source: "登记操作 · 开发 fixture",
    targetLabel: "测试设备 / 当前用户",
    impact: "仅修改测试内存，不操作真实系统",
    current: {
      name: "当前名称",
      enabled: false,
      address: "",
      port: 8080,
      network: "user",
      confirm: false,
      power: "ac",
      path: "",
      time: "",
      organization: "示例组织",
    },
    proposal: { name: "建议名称" },
    phase: template === "TaskExecution" ? "running" : "ready",
    allowed:
      template === "TaskExecution"
        ? ["cancel", "refresh", "answer"]
        : ["submit", "cancel", "refresh", "answer"],
    approval: {
      user: "待确认",
      enterprise: "组织已允许此测试范围",
      elevation: "不需要",
    },
    objects: [
      {
        id: "first",
        label: "测试对象 A",
        detail: "稳定对象 ID first",
        status: "未执行",
      },
      {
        id: "second",
        label: "测试对象 B",
        detail: "稳定对象 ID second",
        disabledReason: "组织限制",
      },
    ],
    logs: [],
    ...(template === "TaskExecution"
      ? {
          progress: { completed: 1, total: 3 },
          interaction: {
            id: "continue",
            question: "是否继续此测试任务？",
            options: [
              { id: "yes", label: "继续" },
              { id: "no", label: "停止" },
            ],
            expiresAtMs: Date.now() + 600000,
          },
        }
      : {}),
  };
  const listeners = new Set<Parameters<OperationHostPort["observe"]>[1]>();
  const secrets = new Map<string, { field: string; value: string }>();
  const events: OperationEvent[] = [];
  let loseReceipt = false;
  const read = () => structuredClone(snapshot);
  const publish = () => {
    snapshot.revision++;
    for (const listener of listeners)
      listener({ snapshot: read(), kind: "projection" });
  };
  const host: OperationHostPort = {
    async read() {
      return read();
    },
    observe(_scope, listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    async dispatch(event) {
      if (
        event.instance !== snapshot.scope.instance ||
        event.revision !== snapshot.revision
      )
        throw new Error("stale fixture binding");
      if (!snapshot.allowed.includes(event.kind))
        throw new Error("fixture denied");
      for (const [field, reference] of Object.entries(event.secretRefs)) {
        if (secrets.get(reference)?.field !== field)
          throw new Error("invalid secret reference");
        secrets.delete(reference);
      }
      events.push(structuredClone(event));
      if (event.kind === "submit") {
        snapshot.current = { ...event.values };
        snapshot.proposal = {};
        snapshot.phase = "succeeded";
        snapshot.result = "fixture 已核实；不是系统执行证据";
        for (const row of snapshot.objects)
          if (event.selection.includes(row.id)) row.status = "已核实";
      } else if (event.kind === "cancel") {
        snapshot.phase = "cancelled";
        snapshot.result = "fixture 已取消";
      } else if (event.kind === "answer") {
        delete snapshot.interaction;
        snapshot.phase = event.answer === "yes" ? "running" : "cancelled";
      }
      snapshot.allowed =
        event.kind === "answer" ? ["cancel", "refresh"] : ["refresh"];
      snapshot.logs.push(`测试动作 ${event.kind} 已处理`);
      publish();
      if (loseReceipt) throw new Error("fixture receipt lost");
      return read();
    },
    async choosePath() {
      return "/fixture/config";
    },
    async captureSecret(_scope, field, value) {
      const reference = crypto.randomUUID();
      secrets.set(reference, { field, value });
      return reference;
    },
    revokeSecret(reference) {
      secrets.delete(reference);
    },
  };
  return {
    presentation,
    snapshot,
    host,
    events,
    secretCount: () => secrets.size,
    update(patch: Partial<OperationSnapshot>) {
      Object.assign(snapshot, patch);
      publish();
    },
    loseReceipt() {
      loseReceipt = true;
    },
  };
}
