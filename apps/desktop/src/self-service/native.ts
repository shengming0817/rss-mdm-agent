import { userGeneration } from "../test-users";
// Only this adapter may access Tauri IPC. Errors never activate a mock service.
import { invoke, isTauri } from "@tauri-apps/api/core";
import type {
  SelfServicePort,
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
