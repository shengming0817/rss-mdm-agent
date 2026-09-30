import fixtures from "../../../../tests/assistant/execution-fixtures.json";
import type { BackendTask, ExecutionTaskDetails, Snapshot } from "./types";
export function executionTask(): ExecutionTaskDetails {
  return structuredClone(fixtures.running) as ExecutionTaskDetails;
}
export function offer(): BackendTask {
  return {
    task: "backend-task",
    attempt: "backend-attempt",
    request: "backend-request",
    revision: "a".repeat(64),
    title: "固定软件 1.0",
    expiresAt: 9999999999,
    userInitiated: true,
  };
}
export function snapshot(): Snapshot {
  return { available: [offer()], requests: [], next: null };
}
