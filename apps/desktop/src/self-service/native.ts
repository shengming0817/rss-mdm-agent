import { userGeneration } from "../test-users";
// Only this adapter may access Tauri IPC. Errors never activate a mock service.
import { invoke, isTauri } from "@tauri-apps/api/core";
import type {
  SelfServicePort,
  Snapshot,
  TaskSubmission,
  BackendTaskView,
} from "./types";
export function nativePort(): SelfServicePort | null {
  if (!isTauri()) return null;
  const generation = userGeneration();
  return {
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
