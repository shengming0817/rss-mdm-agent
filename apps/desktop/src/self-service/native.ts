import { userGeneration } from "../test-users";
// Only this adapter may access Tauri IPC. Errors never activate a mock service.
import { invoke, isTauri } from "@tauri-apps/api/core";
import type {
  SelfServicePort,
  SettingsOperation,
  SettingsResponse,
  SoftwarePort,
  DataAccessPort,
  TaskSubmission,
} from "./types";
export function nativePort(): SelfServicePort | null {
  if (!isTauri()) return null;
  const generation = userGeneration();
  return {
    offers: () => invoke("self_service_offers", { input: null, generation }),
    policy_resolve: (input) =>
      invoke("self_service_policy_resolve", { input, generation }),
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
    execute: (input) =>
      invoke<TaskSubmission>("self_service_execute", { input, generation }),
    confirm: (input) =>
      invoke<TaskSubmission>("self_service_confirm", { input, generation }),
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

export type NetworkCheckPort = (
  input: import("./types").NetworkCheckOperation,
) => Promise<import("./types").NetworkCheckResponse>;
export function nativeNetworkCheckPort(): NetworkCheckPort | null {
  if (!isTauri()) return null;
  const generation = userGeneration();
  return (input) => invoke("network_check", { input, generation });
}

/** Same authenticated task port used by desktop, home and AI cards. */
export function nativeTaskPort(): import("./types").TaskPort | null {
  if (!isTauri()) return null;
  const generation = userGeneration();
  const current = async <T>(pending: Promise<T>): Promise<T> => {
    const result = await pending;
    if (generation !== userGeneration()) throw Error("native scope changed");
    return result;
  };
  return {
    list: (input) => current(invoke("task_list", { input, generation })),
    detail: (input) => current(invoke("task_detail", { input, generation })),
    control: (input) => current(invoke("task_control", { input, generation })),
    controlStatus: (input) =>
      current(invoke("task_control_status", { input, generation })),
  };
}
