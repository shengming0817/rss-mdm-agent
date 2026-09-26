import { userGeneration } from "../test-users";
// Only this adapter may access Tauri IPC. Errors never activate a mock service.
import { invoke, isTauri } from "@tauri-apps/api/core";
import type { SelfServicePort, Snapshot, RequestView } from "./types";
export function nativePort(): SelfServicePort | null {
  if (!isTauri()) return null;
  const generation = userGeneration();
  return {
    snapshot: (input) =>
      invoke<Snapshot>("self_service_snapshot", { input, generation }),
    execute: (input) =>
      invoke<RequestView>("self_service_execute", { input, generation }),
    cancel: (input) =>
      invoke<RequestView>("self_service_cancel", { input, generation }),
    confirm: (input) =>
      invoke<RequestView>("self_service_confirm", { input, generation }),
    respond: (input) =>
      invoke<RequestView>("self_service_respond", { input, generation }),
  };
}
