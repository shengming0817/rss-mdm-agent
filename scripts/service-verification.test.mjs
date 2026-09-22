import { test } from "node:test";
import assert from "node:assert/strict";
import { serviceChecks, verifiedCandidate } from "./verify-local-service.mjs";
const healthy = {
  phase: "connected",
  status: {
    version: 1,
    installation: "a",
    build: "b",
    platform: "fixture",
    capability: "statusOnly",
  },
};
function run(responses) {
  return serviceChecks("desktop", "probe", () => {
    const value = responses.shift();
    return {
      status: value?.error ? 1 : 0,
      stdout: JSON.stringify(value),
      stderr: "",
      pid: 1,
    };
  });
}
test("service absence cannot pass a negative fixture", () => {
  const result = run([{ phase: "notInstalled" }, { admitted: false }, healthy]);
  assert.equal(result.passed, false);
  assert.equal(result.steps.length, 1);
});
test("negative result requires matching healthy bookends", () => {
  assert.equal(run([healthy, { admitted: false }, healthy]).passed, true);
  assert.equal(
    run([healthy, { admitted: false }, { phase: "unavailable" }]).passed,
    false,
  );
  assert.equal(run([healthy, { admitted: true }, healthy]).passed, false);
  assert.equal(
    run([
      healthy,
      { admitted: false },
      { ...healthy, status: { ...healthy.status, installation: "other" } },
    ]).passed,
    false,
  );
});
test("caller-selected programs cannot pass without protected candidate provenance", () => {
  const source = { head: "a".repeat(40), clean: true };
  const fakePolicy = {
    version: 1,
    sourceSha: source.head,
    client: { path: "/fake/desktop", sha256: "b".repeat(64) },
    probe: { path: "/fake/probe", sha256: "c".repeat(64) },
  };
  assert.equal(
    verifiedCandidate(fakePolicy, source, () => "d".repeat(64)),
    undefined,
  );
  assert.equal(
    verifiedCandidate(
      { ...fakePolicy, sourceSha: "e".repeat(40) },
      source,
      (path) => (path.endsWith("desktop") ? "b".repeat(64) : "c".repeat(64)),
    ),
    undefined,
  );
});
