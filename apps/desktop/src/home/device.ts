import { shallowReactive } from "vue";
import type { DeviceOverview } from "../self-service/types";
export type DeviceOverviewPort = { read(): Promise<DeviceOverview> };
export function createDeviceOverview(port?: DeviceOverviewPort) {
  const state = shallowReactive<{
    value: DeviceOverview | null;
    loading: boolean;
  }>({ value: null, loading: false });
  let disposed = false;
  async function refresh() {
    if (!port || disposed || state.loading) return;
    state.loading = true;
    try {
      const value = await port.read();
      if (!disposed) state.value = value;
    } catch {
      // A failed read must not leave an old OS account/address looking current.
      if (!disposed) state.value = null;
    } finally {
      if (!disposed) state.loading = false;
    }
  }
  function dispose() {
    disposed = true;
    state.value = null;
    state.loading = false;
  }
  return { state, refresh, dispose };
}
