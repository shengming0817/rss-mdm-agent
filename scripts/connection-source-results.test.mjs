import assert from "node:assert/strict";
import { test } from "node:test";
import { sourceSummary } from "./connection-source-results.mjs";
test("both official existing configurations require a completed probe; absent sources are not proof of support", () => {
  for (const provider of ["claude", "codex"]) {
    const row = {
      provider,
      source: "existing_config",
      result: "model_probe_completed",
    };
    assert.equal(sourceSummary([row]).status, "passed");
    assert.equal(
      sourceSummary([{ ...row, result: "unsupported_capability" }]).status,
      "failed",
    );
    assert.equal(
      sourceSummary([{ ...row, result: "authentication_required" }]).status,
      "failed",
    );
    assert.equal(
      sourceSummary([{ ...row, result: "source_absent" }]).status,
      "partial",
    );
  }
});
