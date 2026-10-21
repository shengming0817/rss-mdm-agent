import { reactive } from "vue";
import {
  operationPresentation,
  operationSnapshot,
  operationEvent,
  OperationRejectedError,
  type OperationPresentation,
  type OperationSnapshot,
  type OperationHostPort,
  type OperationEvent,
  type OperationValue,
} from "@rss-mdm-agent/ai-contract/operations";

type Values = Record<string, OperationValue>;
const scopeKey = (scope: OperationSnapshot["scope"]) =>
  JSON.stringify([
    scope.authority,
    scope.tenant,
    scope.principal,
    scope.session,
    scope.target,
    scope.instance,
  ]);
interface Draft {
  values: Values;
  dirty: Set<string>;
  baseline: string;
  schema: string;
  selection: string[];
  schemaReset: boolean;
}
interface Execution {
  snapshot: OperationSnapshot;
  busy: boolean;
  unknown: boolean;
  unknownRevision: number;
}
/** Owned by the application, not a renderer or localStorage. */
export class OperationDrafts {
  private entries = new Map<string, Draft>();
  private executions = new Map<string, Execution>();
  execution(snapshot: OperationSnapshot): Execution {
    const key = scopeKey(snapshot.scope);
    let entry = this.executions.get(key);
    if (!entry) {
      entry = reactive({
        snapshot,
        busy: false,
        unknown: false,
        unknownRevision: -1,
      });
      this.executions.set(key, entry);
    }
    return entry;
  }
  acquire(
    snapshot: OperationSnapshot,
    presentation: OperationPresentation,
  ): Draft {
    const key = scopeKey(snapshot.scope);
    // Parameter compatibility is independent of layout and runtime readonly reasons.
    const schema = JSON.stringify({
      version: presentation.version,
      fields: presentation.fields.map((f) => ({
        id: f.id,
        component: f.component,
        required: f.required,
        min: f.min,
        max: f.max,
        maxLength: f.maxLength,
        options: f.options?.map((option) => option.id),
        submitMode: f.submitMode,
      })),
      rules: presentation.rules,
    });
    let draft = this.entries.get(key);
    if (!draft || draft.schema !== schema) {
      const schemaReset = !!draft;
      draft = reactive({
        values: {},
        dirty: new Set<string>(),
        baseline: snapshot.baseline,
        schema,
        selection: [],
        schemaReset,
      });
      this.entries.set(key, draft);
    }
    return draft;
  }
  clear() {
    this.entries.clear();
    this.executions.clear();
  }
}
const fallback: OperationPresentation = {
  version: 1,
  template: "OperationApproval",
  fields: [],
  rules: [],
  sections: [],
};
const text = (v: OperationValue | undefined) =>
  v === undefined || v === null ? "" : String(v);
export function createOperationController(
  input: unknown,
  initial: OperationSnapshot,
  host: OperationHostPort,
  drafts: OperationDrafts,
  now = Date.now,
) {
  let presentation: OperationPresentation,
    fallbackReason = "";
  try {
    presentation = operationPresentation(input);
  } catch {
    presentation = fallback;
    fallbackReason = "专用界面不可用，已显示默认审批";
  }
  initial = operationSnapshot(initial);
  const scope = initial.scope;
  const draft = drafts.acquire(operationSnapshot(initial), presentation);
  const execution = drafts.execution(initial);
  const state = reactive({
    get snapshot() {
      return execution.snapshot;
    },
    set snapshot(value: OperationSnapshot) {
      execution.snapshot = value;
    },
    draft,
    errors: {} as Record<string, string>,
    get busy() {
      return execution.busy;
    },
    set busy(value: boolean) {
      execution.busy = value;
    },
    get unknown() {
      return execution.unknown;
    },
    set unknown(value: boolean) {
      if (value) execution.unknownRevision = execution.snapshot.revision;
      execution.unknown = value;
    },
    get stale() {
      return draft.baseline !== execution.snapshot.baseline;
    },
    notice: draft.schemaReset
      ? "表单版本已变化，请重新填写；未转换旧草稿。"
      : "",
    fallbackReason,
    step: 0,
    interactionExpired: false,
    preparationExpired: false,
    secrets: {} as Record<string, string>,
    capturing: new Set<string>(),
  });
  let disposed = false;
  let deadline: ReturnType<typeof setTimeout> | undefined;
  const captures = new Map<string, number>();
  const fields = new Map(presentation.fields.map((f) => [f.id, f]));
  const secretFields = new Set(initial.secretFields ?? []);
  function merge(
    snapshot: OperationSnapshot,
    acknowledge = false,
    queried = false,
  ) {
    const next = operationSnapshot(snapshot);
    if (scopeKey(next.scope) !== scopeKey(scope))
      throw new Error("操作主体或目标不匹配");
    if (next.revision < state.snapshot.revision) return false;
    secretFields.clear();
    for (const id of next.secretFields ?? []) {
      secretFields.add(id);
      delete draft.values[id];
      draft.dirty.delete(id);
    }
    state.snapshot = next;
    if (
      queried &&
      next.revision > execution.unknownRevision &&
      !state.busy &&
      state.unknown &&
      next.phase !== "unknown"
    ) {
      state.unknown = false;
      state.notice = "";
    }
    if (
      acknowledge &&
      !["failed", "denied", "unknown", "cancelled"].includes(next.phase)
    ) {
      draft.dirty.clear();
      draft.baseline = next.baseline;
    }
    clearTimeout(deadline);
    state.preparationExpired =
      next.expiresAtMs !== undefined && now() >= next.expiresAtMs;
    const interaction = next.interaction;
    state.interactionExpired = !!interaction && now() > interaction.expiresAtMs;
    const deadlines = [interaction?.expiresAtMs, next.expiresAtMs].filter(
      (value): value is number => value !== undefined && value >= now(),
    );
    if (!disposed && deadlines.length)
      deadline = setTimeout(
        () => {
          state.preparationExpired =
            next.expiresAtMs !== undefined && now() >= next.expiresAtMs;
          state.interactionExpired =
            !!interaction && now() > interaction.expiresAtMs;
        },
        Math.min(2147483647, Math.max(1, Math.min(...deadlines) - now() + 1)),
      );
    const values = { ...next.current, ...next.proposal };
    for (const id of Object.keys(draft.values)) {
      if (!(id in values) && !draft.dirty.has(id)) delete draft.values[id];
    }
    for (const [id, value] of Object.entries(values)) {
      if (
        !secretFields.has(id) &&
        (!fields.size || fields.has(id)) &&
        !draft.dirty.has(id)
      )
        draft.values[id] = fields.get(id)?.readonly
          ? (next.current[id] ?? next.proposal[id] ?? null)
          : value;
    }
    if (state.preparationExpired)
      state.notice = "原准备已过期；请核对原请求，草稿已保留。";
    if (next.phase !== "ready") clearSecrets();
    // A value represented by a secret field can never enter generic draft or projection data.
    for (const id of secretFields) {
      delete draft.values[id];
      delete state.snapshot.current[id];
      delete state.snapshot.proposal[id];
    }
    return true;
  }
  merge(
    execution.snapshot.revision > initial.revision
      ? execution.snapshot
      : initial,
  );
  function can(kind: OperationEvent["kind"]) {
    return (
      !disposed &&
      !state.busy &&
      (kind !== "submit" ||
        (!state.capturing.size &&
          !state.preparationExpired &&
          (state.snapshot.expiresAtMs === undefined ||
            now() < state.snapshot.expiresAtMs))) &&
      state.snapshot.allowed.includes(kind) &&
      (kind !== "answer" ||
        (!!state.snapshot.interaction &&
          !state.interactionExpired &&
          now() <= state.snapshot.interaction.expiresAtMs)) &&
      (kind === "refresh" ||
        (!state.unknown &&
          (kind !== "submit" || !state.stale) &&
          !["submitting", "unknown"].includes(state.snapshot.phase)))
    );
  }
  function editable(id: string) {
    const f = fields.get(id);
    return (
      !!f &&
      !disposed &&
      !state.busy &&
      !state.unknown &&
      !state.preparationExpired &&
      !f.readonly &&
      !f.reason &&
      state.snapshot.allowed.includes("submit") &&
      state.snapshot.phase === "ready"
    );
  }
  function validate(ids?: string[]) {
    const errors: Record<string, string> = {};
    for (const f of presentation.fields) {
      const value = draft.values[f.id],
        absent = value === undefined || value === null || value === "";
      if (
        f.required &&
        (secretFields.has(f.id) ? !state.secrets[f.id] : absent)
      )
        errors[f.id] = "请填写此项";
      if (
        !absent &&
        f.component === "NumberField" &&
        (typeof value !== "number" ||
          !Number.isFinite(value) ||
          (f.min !== undefined && value < f.min) ||
          (f.max !== undefined && value > f.max))
      )
        errors[f.id] = "数值超出允许范围";
      if (
        !absent &&
        ["Switch", "Checkbox"].includes(f.component) &&
        typeof value !== "boolean"
      )
        errors[f.id] = "请选择开关状态";
      if (
        !absent &&
        ["TextField", "PathPicker", "DateTimeField"].includes(f.component) &&
        (typeof value !== "string" || value.length > (f.maxLength ?? 8192))
      )
        errors[f.id] = "文本格式或长度无效";
      if (
        !absent &&
        f.component === "DateTimeField" &&
        !Number.isFinite(Date.parse(text(value)))
      )
        errors[f.id] = "日期时间无效";
      if (
        !absent &&
        f.options &&
        !f.options.some((o) => o.id === value && !o.disabledReason)
      )
        errors[f.id] = "请选择可用选项";
    }
    for (const r of presentation.rules) {
      if (
        draft.values[r.when] === r.equals &&
        (r.kind === "required"
          ? !text(draft.values[r.field])
          : draft.values[r.field] !== draft.values[r.when])
      )
        errors[r.field] = r.message;
    }
    state.errors = errors;
    return !Object.keys(errors).some((id) => !ids || ids.includes(id));
  }
  function edit(id: string, value: OperationValue) {
    if (!editable(id) || secretFields.has(id)) return;
    draft.values[id] = value;
    draft.dirty.add(id);
    validate();
    if (fields.get(id)?.submitMode === "immediate") void act("submit");
  }
  function select(id: string) {
    if (
      !can("submit") ||
      !state.snapshot.objects.some((o) => o.id === id && !o.disabledReason)
    )
      return;
    draft.selection =
      presentation.selection === "single"
        ? [id]
        : draft.selection.includes(id)
          ? draft.selection.filter((v) => v !== id)
          : [...draft.selection, id];
  }
  function clearSecrets() {
    for (const [id, ticket] of captures) captures.set(id, ticket + 1);
    state.capturing.clear();
    for (const ref of Object.values(state.secrets)) host.revokeSecret?.(ref);
    state.secrets = {};
  }
  async function secret(id: string, value: string) {
    if (
      !editable(id) ||
      !secretFields.has(id) ||
      !host.captureSecret ||
      !host.revokeSecret
    )
      return;
    const ticket = (captures.get(id) ?? 0) + 1;
    captures.set(id, ticket);
    if (state.secrets[id]) host.revokeSecret(state.secrets[id]);
    delete state.secrets[id];
    state.capturing.add(id);
    try {
      const ref = await host.captureSecret(scope, id, value);
      if (disposed || ticket !== captures.get(id)) {
        host.revokeSecret(ref);
        return;
      }
      state.secrets[id] = ref;
      validate();
    } catch {
      if (!disposed && ticket === captures.get(id))
        state.notice = "秘密输入不可用，请重新输入";
    } finally {
      if (ticket === captures.get(id)) state.capturing.delete(id);
    }
  }
  async function choosePath(id: string) {
    if (
      !editable(id) ||
      fields.get(id)?.component !== "PathPicker" ||
      !host.choosePath
    )
      return;
    try {
      const path = await host.choosePath(scope, id);
      if (path !== undefined && !disposed) edit(id, path);
    } catch {
      state.notice = "路径选择不可用";
    }
  }
  async function act(kind: OperationEvent["kind"], answer?: string) {
    if (!can(kind) || (kind === "submit" && !validate())) return;
    const interaction = state.snapshot.interaction;
    if (
      kind === "answer" &&
      (!interaction ||
        now() > interaction.expiresAtMs ||
        !interaction.options.some((o) => o.id === answer))
    )
      return;
    if (
      kind === "submit" &&
      ["ObjectActions", "ResourceSelection"].includes(presentation.template) &&
      !draft.selection.length
    ) {
      state.notice = "请选择操作对象";
      return;
    }
    if (
      kind === "submit" &&
      draft.selection.some(
        (id) =>
          !state.snapshot.objects.some((o) => o.id === id && !o.disabledReason),
      )
    ) {
      state.notice = "所选对象已不可用，请重新选择";
      return;
    }
    const event = operationEvent({
      id: crypto.randomUUID(),
      instance: scope.instance,
      revision: state.snapshot.revision,
      kind,
      values: kind === "submit" ? JSON.parse(JSON.stringify(draft.values)) : {},
      selection: kind === "submit" ? [...draft.selection] : [],
      secretRefs: kind === "submit" ? { ...state.secrets } : {},
      ...(kind === "answer" ? { interactionId: interaction!.id, answer } : {}),
    });
    if (kind === "cancel") clearSecrets();
    state.busy = true;
    state.notice = "";
    try {
      const snapshot =
        kind === "refresh"
          ? await host.read(scope)
          : await host.dispatch(event, scope);
      if (!merge(snapshot, kind === "submit"))
        throw new Error("操作状态已过期");
      state.unknown = snapshot.phase === "unknown";
      if (!state.unknown) state.notice = "";
      if (kind === "submit") {
        clearSecrets();
      }
    } catch (error) {
      if (error instanceof OperationRejectedError) {
        state.unknown = false;
        state.notice = error.message;
        state.snapshot = {
          ...state.snapshot,
          phase: "denied",
          allowed: state.snapshot.allowed.filter(
            (action) => action === "refresh" || action === "cancel",
          ),
        };
      } else {
        state.unknown = kind !== "refresh" || state.unknown;
        state.notice =
          kind === "refresh"
            ? "无法查询操作状态"
            : "结果未知，请查询原操作，勿重复提交";
      }
      clearSecrets();
    } finally {
      state.busy = false;
    }
  }
  const stop = host.observe(scope, ({ snapshot, kind }) => {
    try {
      merge(snapshot, false, kind === "query");
    } catch {
      state.notice = "操作状态无法核实";
      state.unknown = true;
    }
  });
  return {
    isSecret: (id: string) => secretFields.has(id),
    presentation,
    state,
    update: (snapshot: OperationSnapshot) => merge(snapshot),
    invalidate() {
      clearSecrets();
      state.unknown = true;
      state.notice = "操作状态无法核实，请查询原操作";
    },
    can,
    editable,
    validate,
    edit,
    select,
    secret,
    choosePath,
    act,
    secretAvailable: !!host.captureSecret && !!host.revokeSecret,
    pathAvailable: !!host.choosePath,
    rebase() {
      if (validate()) {
        draft.baseline = state.snapshot.baseline;
      }
    },
    next() {
      const step = presentation.steps?.[state.step];
      if (step && validate(step.fields))
        state.step = Math.min(
          state.step + 1,
          (presentation.steps?.length ?? 1) - 1,
        );
    },
    previous() {
      state.step = Math.max(0, state.step - 1);
    },
    fallback() {
      clearSecrets();
      state.fallbackReason = "专用界面渲染失败，已显示默认审批";
    },
    dispose() {
      disposed = true;
      clearTimeout(deadline);
      clearSecrets();
      stop();
    },
  };
}
export type OperationController = ReturnType<typeof createOperationController>;
