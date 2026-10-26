import assert from "node:assert/strict";
import { test } from "node:test";
import { operationRequestId } from "../../packages/execution-bindings/dist/index.js";

test("backend task references retain the server-selected request and reject old proposals", () => {
  const selection = {
    task: "task",
    attempt: "attempt",
    revision: "a".repeat(64),
    request: "backend-request",
  };
  assert.equal(
    operationRequestId("execution_execute", selection),
    "backend-request",
  );
  for (const input of [
    null,
    {},
    { ...selection, approved: true },
    { ...selection, sourceUtf8: "echo forged" },
    { catalog: { selection: { operationRequestId: "old" } } },
    {
      script: {
        operationRequestId: "old",
        sourceUtf8: "echo old",
        interpreter: {},
      },
    },
  ]) {
    assert.equal(operationRequestId("execution_execute", input), undefined);
  }
  const reference = { kind: "request", request: "backend-request" };
  assert.equal(
    operationRequestId("execution_task_detail", reference),
    "backend-request",
  );
  const control = {
    reference,
    id: "control-original",
    scope: "native-scope",
    expectedRevision: { owner: 1, transfers: [] },
    action: "cancel",
  };
  assert.equal(
    operationRequestId("execution_task_control", control),
    "backend-request",
  );
  assert.equal(
    operationRequestId("execution_task_control", {
      ...control,
      actor: "forged",
    }),
    undefined,
  );
  assert.equal(
    operationRequestId("execution_task_control_status", {
      reference,
      id: control.id,
      scope: control.scope,
    }),
    "backend-request",
  );
});
