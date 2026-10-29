import { proxy, type Snapshot } from "valtio/vanilla";
import { deepClone } from "valtio/vanilla/utils";
import type {
  PolicyChoice,
  PolicyStatus,
  SelfServiceDirectoryItem,
  SelfServicePort,
} from "./types";

/** Human selections stay local until explicit submission; the service owns execution. */
export function createPolicyPreparation(
  port: SelfServicePort | null,
  identity: () => string = () => crypto.randomUUID(),
) {
  const state = proxy({
    item: null as SelfServiceDirectoryItem | null,
    choice: null as PolicyChoice | null,
    status: null as PolicyStatus | null,
    busy: false,
    error: "",
  });
  let active = true,
    epoch = 0;
  async function select(item: SelfServiceDirectoryItem) {
    // An unresolved submission must remain addressable before another selection.
    if (!active || !port || state.busy || state.choice) return;
    const call = ++epoch;
    state.busy = true;
    state.item = null;
    state.status = null;
    state.error = "";
    try {
      const current = await port.policy_resolve({
        kind: "id",
        id: item.policyId,
      });
      if (!active || call !== epoch) return;
      if (
        current.policyId !== item.policyId ||
        current.policyVersion !== item.policyVersion ||
        current.policyRevision !== item.policyRevision ||
        JSON.stringify(current.subject) !== JSON.stringify(item.subject)
      ) {
        state.error = "发布内容或可见主体已变化，请刷新目录后重新选择。";
        return;
      }
      state.item = deepClone(current);
    } catch {
      if (active && call === epoch)
        state.error = "当前发布项目不可用，请刷新目录或检查连接。";
    } finally {
      if (active && call === epoch) state.busy = false;
    }
  }
  const canRestart = (status: Snapshot<PolicyStatus> | null = state.status) => {
    if (!state.choice || !status) return false;
    return status.kind === "preparation"
      ? ["failed", "cancelled"].includes(status.value.state)
      : status.kind === "run" &&
          ["succeeded", "failed"].includes(status.value.state.execution);
  };
  async function restart() {
    if (!active || !port || state.busy || !state.item || !canRestart()) return;
    const call = ++epoch,
      policyId = state.item.policyId;
    state.busy = true;
    state.error = "";
    try {
      const current = await port.policy_resolve({ kind: "id", id: policyId });
      if (!active || call !== epoch) return;
      if (current.policyId !== policyId) throw Error("changed policy");
      state.item = deepClone(current);
      state.choice = null;
      state.status = null;
    } catch {
      if (active && call === epoch)
        state.error = "无法重新读取当前发布项目，原请求仍可查询。";
    } finally {
      if (active && call === epoch) state.busy = false;
    }
  }
  async function recover() {
    const request = state.choice?.request;
    if (!active || !port || !request || state.busy) return;
    const call = epoch;
    state.busy = true;
    state.error = "";
    try {
      const status = await port.policy_status({ requestId: request });
      if (!active || call !== epoch) return;
      state.status = deepClone(status);
      if (status.kind === "absent")
        state.error =
          "尚未找到原请求。可用相同请求与参数重新提交，避免重复操作。";
    } catch {
      if (active && call === epoch)
        state.error = "原请求结果尚未确认，请稍后查询。";
    } finally {
      if (active && call === epoch) state.busy = false;
    }
  }
  async function submit(parameters: Record<string, unknown>) {
    if (!active || !port || !state.item || state.busy) return;
    if (state.choice && state.status?.kind !== "absent") return;
    const choice = state.choice
      ? deepClone(state.choice)
      : {
          request: identity(),
          policyId: state.item.policyId,
          parameters: deepClone(parameters),
        };
    // Retain the immutable input before the first write, including a lost receipt.
    state.choice = choice;
    state.status = null;
    state.busy = true;
    state.error = "";
    const call = epoch;
    try {
      const value = await port.policy_invoke({ choice });
      if (active && call === epoch)
        state.status = { kind: "preparation", value: deepClone(value) };
    } catch {
      if (active && call === epoch)
        state.error = "提交回执未确认，请查询原请求，不要重复发起。";
    } finally {
      if (active && call === epoch) state.busy = false;
    }
  }
  async function confirm() {
    const request = state.choice?.request;
    if (
      !active ||
      !port ||
      !request ||
      state.busy ||
      state.status?.kind !== "preparation" ||
      state.status.value.state !== "awaitingConfirmation"
    )
      return;
    const call = epoch;
    state.busy = true;
    state.error = "";
    try {
      const value = await port.policy_confirm({ requestId: request });
      if (active && call === epoch)
        state.status = { kind: "preparation", value: deepClone(value) };
    } catch {
      if (active && call === epoch)
        state.error = "确认回执未取得，请查询原请求。";
    } finally {
      if (active && call === epoch) state.busy = false;
    }
  }
  function clear() {
    if (
      state.busy ||
      (state.choice &&
        (!state.status ||
          state.status.kind === "absent" ||
          (state.status.kind === "run" &&
            state.status.value.state.execution === "unknown")))
    )
      return false;
    ++epoch;
    state.item = null;
    state.choice = null;
    state.status = null;
    state.error = "";
    return true;
  }
  return {
    state,
    available: !!port,
    select,
    submit,
    recover,
    canRestart,
    restart,
    confirm,
    clear,
    dispose() {
      active = false;
      ++epoch;
      state.item = null;
      state.choice = null;
      state.status = null;
      state.error = "";
      state.busy = false;
    },
  };
}
export type PolicyPreparation = ReturnType<typeof createPolicyPreparation>;
