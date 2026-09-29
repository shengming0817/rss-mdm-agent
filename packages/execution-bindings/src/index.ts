import validExecute from "./validate-execute.js";
import validOperation from "./validate-operation.js";
import type { ExecuteInput } from "./execute-input.js";
import type { OperationRequest } from "./operation-request.js";
export type { ExecuteInput, OperationRequest };

/** Rust-owned request syntax only. This never grants execution or read authority. */
export function operationRequestId(
  name: string,
  input: unknown,
): string | undefined {
  if (name === "execution_execute") {
    if (!validExecute(input)) return undefined;
    const request = input as ExecuteInput;
    return "catalog" in request
      ? request.catalog.selection.operationRequestId
      : request.script.operationRequestId;
  }
  if (name === "execution_status" || name === "execution_cancel") {
    if (!validOperation(input)) return undefined;
    return (input as OperationRequest).operationRequestId;
  }
  return undefined;
}
// Keep provider tool definitions as the same generated artifact, including in deployed packages.
export { default as tools } from "./tools.json" with { type: "json" };
