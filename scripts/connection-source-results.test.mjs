import assert from "node:assert/strict";
import { test } from "node:test";
import { sourceSummary } from "./connection-source-results.mjs";
test("explicit unsupported login is verified while absent supported sources remain not applicable", () => {
  const blocked = {
    provider: "claude",
    source: "existing_login",
    result: "unsupported_capability",
  };
  assert.equal(sourceSummary([blocked]).status, "passed");
  assert.equal(
    sourceSummary([{ ...blocked, result: "model_probe_completed" }]).status,
    "failed",
  );
  assert.equal(
    sourceSummary([{ ...blocked, result: "source_absent" }]).status,
    "failed",
  );
  const missing = {
    provider: "codex",
    source: "existing_login",
    result: "source_absent",
  };
  const partial = sourceSummary([blocked, missing]);
  assert.equal(partial.status, "partial");
  assert.equal(partial.results[1].status, "not_applicable");
  assert.equal(
    sourceSummary([blocked, { ...missing, result: "model_probe_completed" }])
      .status,
    "passed",
  );
  assert.equal(
    sourceSummary([blocked, { ...missing, result: "authentication_required" }])
      .status,
    "failed",
  );
});
