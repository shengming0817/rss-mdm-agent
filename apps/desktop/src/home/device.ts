import type { DeviceOverview } from "../self-service/types";
export type DeviceOverviewPort = { read(): Promise<DeviceOverview> };
export function createDeviceOverview(port?: DeviceOverviewPort) {
  const state = { value: null, loading: false, error: "", readAt: null } as {
    value: DeviceOverview | null;
    loading: boolean;
    error: string;
    readAt: number | null;
  };
  let disposed = false;
  async function refresh() {
    if (!port || disposed || state.loading) return;
    state.loading = true;
    try {
      const value = await port.read();
      if (!disposed) {
        state.value = value;
        state.readAt = Date.now();
        state.error = "";
      }
    } catch {
      // A failed read must not leave an old OS account/address looking current.
      if (!disposed) {
        state.value = null;
        state.error = "设备概览读取失败";
        state.readAt = null;
      }
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
