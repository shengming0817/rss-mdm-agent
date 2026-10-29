import { proxy } from "valtio/vanilla";
import { deepClone } from "valtio/vanilla/utils";
import type {
  PersonalVaultOperation,
  PersonalVaultPort,
  PersonalVaultStatus,
} from "./native";
export function createPersonalVault(port?: PersonalVaultPort) {
  const state = proxy({
    value: null as PersonalVaultStatus | null,
    busy: false,
    error: "",
  });
  let active = true;
  async function run(input: PersonalVaultOperation) {
    if (!active || !port || state.busy) return;
    state.busy = true;
    state.error = "";
    try {
      const value = await port(input);
      if (active) state.value = deepClone(value);
    } catch {
      if (active)
        state.error = "个人 Passkey 操作未确认或已取消，请重读状态后核对。";
    } finally {
      if (active) state.busy = false;
    }
  }
  return {
    state,
    available: !!port,
    run,
    dispose() {
      active = false;
      state.value = null;
      state.busy = false;
      state.error = "";
    },
  };
}
