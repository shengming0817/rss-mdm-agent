import validExecute from "./validate-execute.js";
import validTaskDetail from "./validate-task-detail-input.js";
import validTaskControl from "./validate-task-control.js";
import validTaskControlStatus from "./validate-task-control-status.js";
import type { BackendSelection } from "./execute-input.js";
import type { TaskDetailQuery } from "./task-detail-input.js";
export type { TaskRef } from "./tasks.js";
import type { TaskControl } from "./task-control.js";
import type { TaskControlQuery } from "./task-control-status.js";
export type {
  BackendSelection,
  TaskDetailQuery,
  TaskControl,
  TaskControlQuery,
};

/** Rust-owned request syntax only. This never grants execution or read authority. */
export function operationRequestId(
  name: string,
  input: unknown,
): string | undefined {
  if (name === "execution_execute") {
    if (!validExecute(input)) return undefined;
    const request = input as BackendSelection;
    return request.request;
  }
  if (name === "execution_task_detail") {
    if (!validTaskDetail(input)) return undefined;
    const reference = (input as TaskDetailQuery).reference;
    return reference.kind === "request" ? reference.request : undefined;
  }
  if (
    name === "execution_task_control" ||
    name === "execution_task_control_status"
  ) {
    if (
      !(name === "execution_task_control"
        ? validTaskControl(input)
        : validTaskControlStatus(input))
    )
      return undefined;
    const reference = (input as TaskControl | TaskControlQuery).reference;
    return reference.kind === "request" ? reference.request : undefined;
  }
  return undefined;
}
// Keep provider tool definitions as the same generated artifact, including in deployed packages.
export { default as tools } from "./tools.json" with { type: "json" };
