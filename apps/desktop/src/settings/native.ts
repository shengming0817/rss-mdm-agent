import { invoke, isTauri } from "@tauri-apps/api/core";
import {
  boundedJson,
  decode,
  accessLimits,
  type HostStatus,
} from "@rss-mdm-agent/ai-contract";
const status = (value: unknown): HostStatus => {
  const record = decode(boundedJson(value, accessLimits), accessLimits);
  if (record.kind !== "hostStatus") throw new Error("invalid_response");
  return record;
};
export function nativeHost() {
  if (!isTauri()) return undefined;
  return {
    async read() {
      return status(await invoke("ai_host_status"));
    },
    async restart(generation: number) {
      return status(await invoke("ai_restart_host", { generation }));
    },
    async export() {
      return invoke<boolean>("ai_export_diagnostics");
    },
  };
}
