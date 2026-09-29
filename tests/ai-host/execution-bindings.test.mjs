import assert from "node:assert/strict";
import { test } from "node:test";
import { operationRequestId } from "../../packages/execution-bindings/dist/index.js";

test("Rust-generated execution requests have one shared checked business identity", () => {
  const catalog = {
    catalog: {
      selection: {
        operationRequestId: "catalog-request",
        catalog: {
          authority: { kind: "test", id: "fixture" },
          identity: { id: "catalog", revision: "1" },
          digest: "0".repeat(64),
        },
        itemId: "item",
        variantId: "variant",
        arguments: {},
      },
    },
  };
  const script = {
    script: {
      operationRequestId: "script-request",
      sourceUtf8: "echo fixture",
      interpreter: {
        resource: { id: "shell", revision: "1" },
        sha256: "0".repeat(64),
      },
    },
  };
  assert.equal(
    operationRequestId("execution_execute", catalog),
    "catalog-request",
  );
  assert.equal(
    operationRequestId("execution_execute", script),
    "script-request",
  );
  for (const input of [
    null,
    {},
    { ...catalog, ...script },
    { script: { operationRequestId: "incomplete" } },
    { ...script, approved: true },
  ])
    assert.equal(operationRequestId("execution_execute", input), undefined);
  for (const name of ["execution_status", "execution_cancel"]) {
    assert.equal(
      operationRequestId(name, { operationRequestId: "request" }),
      "request",
    );
    assert.equal(
      operationRequestId(name, {
        operationRequestId: "request",
        approved: true,
      }),
      undefined,
    );
  }
  assert.equal(operationRequestId("unrecognized", script), undefined);
});
