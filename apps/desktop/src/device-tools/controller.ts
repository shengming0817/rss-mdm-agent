import { proxy } from "valtio/vanilla";
import { deepClone } from "valtio/vanilla/utils";
import type { NetworkCheckPort } from "../self-service/native";
import type { NetworkCheckStatus, NetworkTarget } from "../self-service/types";

/** One immutable network check; loss of a receipt never starts a second check. */
export function createNetworkCheck(
  port: NetworkCheckPort | null,
  identity: () => string = () => crypto.randomUUID(),
) {
  const state = proxy({
    defaults: null as NetworkTarget | null,
    request: null as string | null,
    target: null as NetworkTarget | null,
    status: null as NetworkCheckStatus | null,
    busy: false,
    error: "",
  });
  let active = true,
    epoch = 0;
  const ended = () =>
    state.status &&
    [
      "verified",
      "failedBeforeDispatch",
      "cancelled",
      "admissionDenied",
      "executionEnded",
    ].includes(state.status.status.phase);
  async function load() {
    if (!active || !port || state.busy) return;
    const call = epoch;
    state.busy = true;
    state.error = "";
    try {
      const result = await port({ kind: "defaults" });
      if (active && call === epoch && result.kind === "defaults")
        state.defaults = deepClone(result.value);
    } catch {
      if (active && call === epoch)
        state.error = "无法读取默认检测目标，可手动填写主机与端口。";
    } finally {
      if (active && call === epoch) state.busy = false;
    }
  }
  async function execute(target?: NetworkTarget) {
    if (!active || !port || state.busy) return;
    const run = !!target;
    if (run && state.request && !ended()) return;
    if (target) {
      state.request = identity();
      state.target = deepClone(target);
      state.status = null;
    }
    if (!state.request || !state.target) return;
    const request = state.request,
      call = epoch;
    state.busy = true;
    state.error = "";
    try {
      const response = await port(
        run
          ? { kind: "run", request, target: deepClone(state.target) }
          : { kind: "status", request },
      );
      if (!active || call !== epoch) return;
      if (response.kind !== "status") throw Error("unexpected response");
      if (response.value.status.operationRequestId !== request)
        throw Error("request mismatch");
      state.status = deepClone(response.value);
    } catch {
      if (active && call === epoch)
        state.error =
          "检测结果未确认，请查询原请求。DNS/TCP 检测不代表应用服务健康。";
    } finally {
      if (active && call === epoch) state.busy = false;
    }
  }
  return {
    state,
    available: !!port,
    load,
    run: (target: NetworkTarget) => execute(target),
    recover: () => execute(),
    ended,
    dispose() {
      active = false;
      ++epoch;
      state.defaults = null;
      state.request = null;
      state.target = null;
      state.status = null;
      state.error = "";
      state.busy = false;
    },
  };
}
