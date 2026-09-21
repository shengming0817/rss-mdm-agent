import { userGeneration } from "../test-users";
// Only this adapter may access Tauri IPC. Errors never activate a mock service.
import { invoke, isTauri } from "@tauri-apps/api/core";
import type { SelfServicePort, Snapshot, Plan, RequestView } from "./types";
export function nativePort(): SelfServicePort | null {
  if (!isTauri()) return null;
  const generation = userGeneration();
  return {
    snapshot: (input) =>
      invoke<Snapshot>("self_service_snapshot", { input, generation }),
    preview: (input) =>
      invoke<Plan>("self_service_preview", { input, generation }),
    submit: (input) =>
      invoke<RequestView>("self_service_submit", { input, generation }),
    cancel: (input) =>
      invoke<RequestView>("self_service_cancel", { input, generation }),
    approve: (input) =>
      invoke<RequestView>("self_service_approve", { input, generation }),
    respond: (input) =>
      invoke<RequestView>("self_service_respond", { input, generation }),
  };
}
