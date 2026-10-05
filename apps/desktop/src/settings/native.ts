import type { ServiceView, RegistrationStatus } from "./service-contract";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { boundedJson, decode, accessLimits } from "@rss-mdm-agent/ai-contract";
import type { HostStatus } from "@rss-mdm-agent/ai-contract/wire";
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

export type ServicePort = {
  read(): Promise<ServiceView>;
  registration(): Promise<RegistrationStatus>;
  requestRegistration(): Promise<RegistrationStatus>;
};
export function nativeService(): ServicePort | undefined {
  if (!isTauri()) return undefined;
  return {
    read: () => invoke<ServiceView>("execution_service_status"),
    registration: () => invoke<RegistrationStatus>("agent_registration_status"),
    requestRegistration: () =>
      invoke<RegistrationStatus>("agent_request_registration"),
  };
}
