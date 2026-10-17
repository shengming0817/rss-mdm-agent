import { invoke, isTauri } from "@tauri-apps/api/core";
import type { DeviceOverview } from "../self-service/types";
import type { DeviceOverviewPort } from "./device";
export function nativeDeviceOverview(): DeviceOverviewPort | undefined {
  if (!isTauri()) return undefined;
  return { read: () => invoke<DeviceOverview>("device_overview") };
}
