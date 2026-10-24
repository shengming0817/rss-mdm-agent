import type {
  ServiceView,
  RegistrationStatus,
  MdmStatus,
} from "./service-contract";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { boundedJson, decode, accessLimits } from "@rss-mdm-agent/ai-contract";
import type { HostStatus } from "@rss-mdm-agent/ai-contract/wire";
import { currentUser, refreshAccount } from "../test-users";
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
      const identity = currentUser.value?.generation;
      try {
        return status(await invoke("ai_restart_host", { generation }));
      } finally {
        if (currentUser.value?.generation === identity) await refreshAccount();
      }
    },
  };
}

export type ServicePort = {
  read(): Promise<ServiceView>;
  registration(): Promise<RegistrationStatus>;
  mdmStatus(): Promise<MdmStatus>;
  requestRegistration(
    input?: import("../account/types").EnterpriseLogin,
  ): Promise<RegistrationStatus>;
  cancelRegistration(): Promise<void>;
  importRegistration(): Promise<RegistrationStatus | null>;
};
export function nativeService(): ServicePort | undefined {
  if (!isTauri()) return undefined;
  return {
    read: () => invoke<ServiceView>("execution_service_status"),
    registration: () => invoke<RegistrationStatus>("agent_registration_status"),
    mdmStatus: readMdmStatus,
    async requestRegistration(input) {
      try {
        return await invoke<RegistrationStatus>("agent_request_registration", {
          input,
        });
      } finally {
        await refreshAccount();
      }
    },
    cancelRegistration: () => invoke<void>("agent_cancel_registration"),
    importRegistration: () =>
      invoke<RegistrationStatus | null>("agent_import_registration"),
  };
}

export async function readMdmStatus(): Promise<MdmStatus> {
  return invoke<MdmStatus>("mdm_registration_status");
}

export type DiagnosticScopes = import("../self-service/types").Scopes;
export type DiagnosticPreview = import("../self-service/types").Preview;
export type DiagnosticPort = {
  prepare(input: DiagnosticScopes): Promise<DiagnosticPreview>;
  export(snapshot: string): Promise<boolean>;
  discard(snapshot: string): Promise<void>;
};
export function nativeDiagnostics(): DiagnosticPort | undefined {
  if (!isTauri()) return undefined;
  return {
    prepare: (input) => invoke("diagnostics_prepare", { input }),
    export: (snapshot) => invoke("diagnostics_export", { input: { snapshot } }),
    discard: (snapshot) =>
      invoke("diagnostics_discard", { input: { snapshot } }),
  };
}
