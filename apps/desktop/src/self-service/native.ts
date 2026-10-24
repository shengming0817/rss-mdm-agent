import { userGeneration } from "../test-users";
// Only this adapter may access Tauri IPC. Errors never activate a mock service.
import { invoke, isTauri } from "@tauri-apps/api/core";
import type {
  SelfServicePort,
  SettingsOperation,
  SettingsResponse,
  SoftwarePort,
  DataAccessPort,
  Snapshot,
  TaskSubmission,
  BackendTaskView,
} from "./types";
export function nativePort(): SelfServicePort | null {
  if (!isTauri()) return null;
  const generation = userGeneration();
  return {
    policy_directory: (input) =>
      invoke("self_service_policy_directory", { input, generation }),
    policy_invoke: (input) =>
      invoke("self_service_policy_invoke", { input, generation }),
    policy_confirm: (input) =>
      invoke("self_service_policy_confirm", { input, generation }),
    policy_status: (input) =>
      invoke("self_service_policy_status", { input, generation }),
    observe_software: (input) =>
      invoke("self_service_observe_software", { input, generation }),
    snapshot: (input) =>
      invoke<Snapshot>("self_service_snapshot", { input, generation }),
    execute: (input) =>
      invoke<TaskSubmission>("self_service_execute", { input, generation }),
    confirm: (input) =>
      invoke<TaskSubmission>("self_service_confirm", { input, generation }),
    cancel: (input) =>
      invoke<BackendTaskView>("self_service_cancel", { input, generation }),
  };
}

/** Shared local data adapter for modules; reads never invoke a collector. */
export function nativeDataPort(): DataAccessPort | null {
  if (!isTauri()) return null;
  const generation = userGeneration();
  return {
    query_directory: (input) =>
      invoke("query_directory", { input, generation }),
    data_read: (input) => invoke("data_read", { input, generation }),
    query_submit: (input) => invoke("query_submit", { input, generation }),
    query_status: (input) => invoke("query_status", { input, generation }),
    query_cancel: (input) => invoke("query_cancel", { input, generation }),
  };
}

/** Read-only software facts and typed local actions from the same authenticated service. */
export function nativeSoftwarePort(): SoftwarePort | null {
  if (!isTauri()) return null;
  const generation = userGeneration();
  return {
    history: (input) => invoke("software_history", { input, generation }),
    read: (input) => invoke("software_read", { input, generation }),
    prepare: (input) => invoke("software_prepare", { input, generation }),
    submit: (input) => invoke("software_submit", { input, generation }),
    status: (input) => invoke("software_status", { input, generation }),
    cancel: (input) => invoke("software_cancel", { input, generation }),
  };
}

export type DeviceAccessPort = (
  input: import("./types").SelfServiceCommands["device_access"]["input"],
) => Promise<import("./types").SelfServiceCommands["device_access"]["output"]>;

/** Bound to the same native employee generation as the other desktop operations. */
export function nativeDeviceAccessPort(): DeviceAccessPort | null {
  if (!isTauri()) return null;
  const generation = userGeneration();
  return (input) => invoke("device_access", { input, generation });
}

/** Finite desktop settings; no equivalent AI write tool is registered. */
export function nativeSettingsPort():
  | ((input: SettingsOperation) => Promise<SettingsResponse>)
  | null {
  if (!isTauri()) return null;
  const generation = userGeneration();
  return (input) =>
    invoke<SettingsResponse>("system_settings", { input, generation });
}
