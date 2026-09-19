// Only this adapter may access Tauri IPC. Errors never activate a mock service.
import { invoke, isTauri } from "@tauri-apps/api/core";
import type { SelfServicePort, Snapshot, Plan, RequestView } from "./types";
export function nativePort(): SelfServicePort | null {
  if (!isTauri()) return null;
  return {
    snapshot: () => invoke<Snapshot>("self_service_snapshot"),
    preview: (input) => invoke<Plan>("self_service_preview", { input }),
    submit: (input) => invoke<RequestView>("self_service_submit", { input }),
    respond: (input) => invoke<RequestView>("self_service_respond", { input }),
  };
}
export function newIdentity(): string {
  return crypto.randomUUID();
}
