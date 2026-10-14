import type { AssistantController } from "../controller";
import {
  OperationRejectedError,
  operationSnapshot,
  type OperationHostPort,
  type OperationSnapshot,
} from "@rss-mdm-agent/ai-contract/operations";
import { operationOrigin, operationProjection, requestId } from "./projection";
import type { BackendTaskView } from "@rss-mdm-agent/execution-bindings/task-details";

// View ordering survives renderer remounts and wall-clock corrections; it is not a journal revision.
let projectionRevision = 0;

/** One view adapter. All durable decisions remain in the authenticated system service. */
export async function connectOperation(
  c: AssistantController,
  operationId: string,
  signal: AbortSignal,
) {
  const session = c.view.value;
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
  let revision = ++projectionRevision;
  const listeners = new Set<(snapshot: OperationSnapshot) => void>();
  const initial = operationProjection(raw, scope, revision);
  let latest = operationSnapshot(initial.snapshot);
  let pending: Promise<OperationSnapshot> | undefined;
  const valid = () => {
    const current = c.view.value;
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
  const accept = (next: BackendTaskView) => {
    valid();
    const actual = operationOrigin(next);
    if (
      requestId(next) !== scope.instance ||
      actual.kind !== "ai" ||
      actual.osSession.device !== scope.target ||
      JSON.stringify(actual) !== JSON.stringify(origin)
    )
      throw Error("操作绑定已变化");
    raw = next;
    revision = ++projectionRevision;
    latest = operationSnapshot(
      operationProjection(raw, scope, revision).snapshot,
    );
    for (const listener of listeners) listener(latest);
    return latest;
  };
  const read = async () => {
    valid();
    if (!pending)
      pending = (async () => {
        if (
          raw.kind === "pending" &&
          raw.value.action.kind === "policy" &&
          raw.value.state !== "draft"
        )
          await c.reconcilePolicyPreparation?.(scope.instance).catch(() => {
            // Reconciliation may need the backend. The authenticated local record
            // remains the source of truth even while that network is unavailable.
          });
        valid();
        return c.executionDetails(operationId, signal);
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
          for (const listener of listeners) listener(latest);
          throw error;
        })
        .finally(() => {
          pending = undefined;
        });
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
      if (event.kind === "submit") {
        if (
          raw.kind !== "pending" ||
          (latest.expiresAtMs !== undefined && Date.now() >= latest.expiresAtMs)
        )
          throw Error("原准备已失效");
        const record = raw.value,
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
