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
test("candidate programs must match installed paths and digests", () => {
  const policy = {
    version: 1,
    client: { path: "/fake/desktop", sha256: "b".repeat(64) },
    probe: { path: "/fake/probe", sha256: "c".repeat(64) },
  };
  const hash = (path) =>
    path.endsWith("desktop") ? "b".repeat(64) : "c".repeat(64);
  assert.deepEqual(verifiedCandidate(policy, hash), {
    executable: "/fake/desktop",
    negative: "/fake/probe",
  });
  assert.equal(
    verifiedCandidate(policy, () => "d".repeat(64)),
    undefined,
  );
  assert.equal(
    verifiedCandidate(
      { ...policy, client: { ...policy.client, path: "relative" } },
      hash,
    ),
    undefined,
  );
});
