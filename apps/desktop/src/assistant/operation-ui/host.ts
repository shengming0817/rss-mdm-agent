import type { AssistantController } from "../controller";
import {
  OperationRejectedError,
  operationSnapshot,
  type OperationHostPort,
  type OperationSnapshot,
} from "@rss-mdm-agent/ai-contract/operations";
import { operationOrigin, operationProjection, requestId } from "./projection";
import type { TaskDetail } from "@rss-mdm-agent/execution-bindings/tasks";

// View ordering survives renderer remounts and wall-clock corrections; it is not a journal revision.
let projectionRevision = 0;

/** One view adapter. All durable decisions remain in the authenticated system service. */
export async function connectOperation(
  c: AssistantController,
  operationId: string,
  signal: AbortSignal,
) {
  const session = c.view();
  if (!session) throw Error("会话不可用");
  let raw = await c.executionDetails(operationId, signal);
  const origin = operationOrigin(raw);
  if (origin.kind !== "ai") throw Error("操作来源无法核实");
  const scope: OperationSnapshot["scope"] = {
    authority: session.namespace.authorityId,
    tenant: session.namespace.tenantId,
    principal: session.namespace.principalId,
    session: session.namespace.sessionId,
    target: origin.osSession.device,
    instance: requestId(raw),
  };
  async function preparationFor(task: TaskDetail) {
    if (
      task.facts.kind !== "preparation" ||
      task.facts.value.action.kind !== "policy"
    )
      return undefined;
    const record = await c.preparationDetails(requestId(task));
    if (
      record.revision !== task.revision.owner ||
      JSON.stringify(record.trigger) !==
        JSON.stringify(task.facts.value.trigger)
    )
      throw Error("准备版本已变化，请重新读取");
    if (
      record.action.kind !== "policy" ||
      record.action.choice.request !== requestId(task)
    )
      throw Error("准备引用不匹配");
    return record;
  }
  let preparation = await preparationFor(raw);
  let revision = ++projectionRevision;
  const listeners = new Set<Parameters<OperationHostPort["observe"]>[1]>();
  const initial = operationProjection(
    raw,
    scope,
    revision,
    Date.now(),
    preparation,
  );
  let latest = operationSnapshot(initial.snapshot);
  let pending: Promise<OperationSnapshot> | undefined;
  let mutationEpoch = 0,
    mutating = false;
  let queriedEpoch: number | undefined = 0;
  const valid = () => {
    const current = c.view();
    if (
      signal.aborted ||
      !current ||
      current.namespace.sessionId !== scope.session ||
      current.namespace.principalId !== scope.principal ||
      current.namespace.tenantId !== scope.tenant ||
      current.namespace.authorityId !== scope.authority
    )
      throw Error("操作上下文已切换");
  };
  const accept = async (next: TaskDetail) => {
    valid();
    const actual = operationOrigin(next);
    if (
      requestId(next) !== scope.instance ||
      actual.kind !== "ai" ||
      actual.osSession.device !== scope.target ||
      JSON.stringify(actual) !== JSON.stringify(origin)
    )
      throw Error("操作绑定已变化");
    const nextPreparation = await preparationFor(next);
    valid();
    raw = next;
    preparation = nextPreparation;
    revision = ++projectionRevision;
    latest = operationSnapshot(
      operationProjection(raw, scope, revision, Date.now(), preparation)
        .snapshot,
    );
    queriedEpoch = mutationEpoch;
    for (const listener of listeners)
      listener({ snapshot: latest, kind: "query" });
    return latest;
  };
  const read = async () => {
    valid();
    if (!pending) {
      const epoch = mutationEpoch;
      const duringMutation = mutating;
      pending = (async () => {
        if (
          raw.facts.kind === "preparation" &&
          raw.facts.value.action.kind === "policy" &&
          raw.facts.value.state !== "draft"
        )
          await c.reconcilePolicyPreparation?.(scope.instance).catch(() => {
            // Reconciliation may need the backend. The authenticated local record
            // remains the source of truth even while that network is unavailable.
          });
        valid();
        const next = await c.executionDetails(operationId, signal);
        if (duringMutation || mutating || mutationEpoch !== epoch)
          throw Error("查询跨越提交，请重新查询原请求");
        return next;
      })()
        .then(accept)
        .catch((error) => {
          revision = ++projectionRevision;
          latest = {
            ...latest,
            revision,
            phase: "unknown",
            reason: "原操作状态暂时无法核实，请查询原请求。",
            allowed: ["refresh"],
          };
          queriedEpoch = undefined;
          for (const listener of listeners)
            listener({ snapshot: latest, kind: "projection" });
          throw error;
        })
        .finally(() => {
          pending = undefined;
        });
    }
    return pending;
  };
  const host: OperationHostPort = {
    read: async (input) => {
      if (JSON.stringify(input) !== JSON.stringify(scope))
        throw Error("操作不匹配");
      return read();
    },
    observe: (_, listener) => {
      listeners.add(listener);
      listener({
        snapshot: latest,
        kind:
          !mutating && queriedEpoch === mutationEpoch ? "query" : "projection",
      });
      return () => listeners.delete(listener);
    },
    async dispatch(event, input) {
      valid();
      if (
        JSON.stringify(input) !== JSON.stringify(scope) ||
        event.instance !== scope.instance ||
        event.revision !== latest.revision ||
        !latest.allowed.includes(event.kind)
      )
        throw Error("操作状态已变化，请刷新");
      mutationEpoch++;
      mutating = true;
      try {
        if (event.kind === "submit") {
          if (
            raw.facts.kind !== "preparation" ||
            (latest.expiresAtMs !== undefined &&
              Date.now() >= latest.expiresAtMs)
          )
            throw Error("原准备已失效");
          const record = preparation ?? raw.facts.value,
            action = record.action;
          if (action.kind === "policy" && record.state === "draft") {
            const parameters = { ...action.choice.parameters };
            for (const binding of initial.bindings)
              if (binding.editable) {
                if (Object.hasOwn(event.values, binding.id))
                  parameters[binding.key] = event.values[binding.id];
                else delete parameters[binding.key];
              }
            try {
              await c.submitOperation({
                request: scope.instance,
                revision: record.revision,
                parameters,
              });
            } catch (error) {
              const reasons: Record<string, string> = {
                operation_invalid_parameters:
                  "参数未通过可信校验；草稿已保留，请刷新后修改。",
                operation_conflict:
                  "操作版本或参数已变化；草稿已保留，请查询原请求。",
                operation_expired: "原准备已过期；草稿已保留，不能沿用旧确认。",
                operation_denied: "当前身份或权限不允许此操作。",
                operation_unsupported: "当前服务不支持此操作。",
              };
              const code =
                error && typeof error === "object" && "code" in error
                  ? String(error.code)
                  : "";
              if (Object.hasOwn(reasons, code))
                throw new OperationRejectedError(reasons[code]);
              throw error;
            }
          } else if (
            action.kind === "policy" &&
            record.state === "awaitingConfirmation"
          )
            await c.confirmPolicyPreparation(scope.instance);
          else if (
            action.kind === "offered" &&
            record.state === "awaitingConfirmation"
          )
            await c.confirmPreparation(action.offer);
          else throw Error("当前状态不允许提交");
        } else if (event.kind === "cancel")
          await c.cancelPreparation(scope.instance);
        else throw Error("此操作不接受普通问答");
      } finally {
        mutating = false;
        mutationEpoch++;
      }
      // In-flight pre-submit reads cannot overwrite the resulting authoritative state.
      if (pending) await pending.catch(() => {});
      return read();
    },
  };
  return {
    current: () => raw,
    host,
    scope,
    snapshot: latest,
    presentation: initial.presentation,
    read,
  };
}
