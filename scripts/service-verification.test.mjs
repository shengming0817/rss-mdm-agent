import { test } from "node:test";
import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { cargoTargetDir } from "./cargo-target.mjs";
import { serviceChecks, verifyLocalService } from "./verify-local-service.mjs";
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
const candidate = {
  version: 1,
  phase: "verified",
  policyPath: "/installed/policy.json",
  policyVersion: 1,
  helperVersion: "0.1.0",
  permissionCheck: "passed",
  executable: "/installed/desktop",
  negative: "/installed/probe",
};
function verify(result, responses = [healthy, { admitted: false }, healthy]) {
  const calls = [],
    receipts = [];
  const receipt = verifyLocalService({
    platform: "darwin",
    execute(program, args) {
      calls.push({ program, args });
      if (calls.length === 1) return result;
      return {
        status: 0,
        stdout: JSON.stringify(responses.shift()),
        stderr: "",
        pid: 2,
      };
    },
    writeReceipt(value) {
      receipts.push(structuredClone(value));
    },
  });
  return { receipt, calls, receipts };
}
const output = (value) => ({
  status: 0,
  stdout: JSON.stringify(value),
  stderr: "",
  pid: 1,
});
test("native policy is the only candidate authority and stale PASS is invalidated before spawn", () => {
  const old = process.env.ProgramData;
  process.env.ProgramData = "/attacker";
  try {
    const { receipt, calls, receipts } = verify(output(candidate));
    assert.equal(receipts[0].status, "failed");
    assert.equal(
      receipt.status,
      "passed-controlled-query-and-negative-fixture",
    );
    assert.equal(receipt.policyPath, candidate.policyPath);
    assert.equal(receipt.helperVersion, "0.1.0");
    assert.equal(receipt.permissionCheck, "passed");
    assert.equal(calls.length, 4);
    assert.equal(
      calls[0].program,
      join(
        cargoTargetDir(fileURLToPath(new URL("../", import.meta.url))),
        "release/rss-local-service",
      ),
    );
    assert.deepEqual(calls[0].args, ["--verification-candidate"]);
    assert.equal(calls[1].program, candidate.executable);
    assert.equal(calls[2].program, candidate.negative);
    assert.equal("artifactSha256" in receipt, false);
    assert.equal("probeSha256" in receipt, false);
  } finally {
    if (old === undefined) delete process.env.ProgramData;
    else process.env.ProgramData = old;
  }
});
test("native rejection, malformed output and process failures never start a candidate", () => {
  const rejected = {
    ...candidate,
    phase: "rejected",
    permissionCheck: "failed",
    executable: undefined,
    negative: undefined,
    reason: "protection",
  };
  for (const result of [
    { status: 1, stdout: JSON.stringify(rejected) },
    output({ ...candidate, version: 0 }),
    output({ ...candidate, permissionCheck: "failed" }),
    output({ ...candidate, executable: "relative" }),
    output({ ...candidate, policyPath: "relative" }),
    output({ ...candidate, helperVersion: "" }),
    output({ ...candidate, policyVersion: 0 }),
    output({ ...candidate, unexpected: "untrusted" }),
    { ...output(candidate), status: 1 },
    { ...output(candidate), signal: "SIGTERM" },
    { ...output(candidate), error: Error("spawn failed") },
    { status: 0, stdout: "not json" },
  ]) {
    const { receipt, calls } = verify(result);
    assert.equal(receipt.status, "failed");
    assert.equal(calls.length, 1);
    assert.deepEqual(receipt.steps, []);
  }
});
test("native exceptions overwrite old pass and cannot escape receipt failure handling", () => {
  const receipts = [];
  const receipt = verifyLocalService({
    execute() {
      throw Error("missing helper");
    },
    writeReceipt(v) {
      receipts.push(structuredClone(v));
    },
  });
  assert.equal(receipt.status, "failed");
  assert.equal(receipts.at(-1).status, "failed");
});

test("process failures retain closed diagnostics and the failing stage", () => {
  for (const failure of [
    { status: null, error: { code: "ENOENT", message: "secret" } },
    { status: null, signal: "SIGTERM", error: { code: "ETIMEDOUT" } },
    { status: null, signal: "SIGKILL" },
    { status: 17 },
    { status: null, error: { code: "secret", message: "secret" } },
  ]) {
    const { receipt } = verify({ ...output(candidate), ...failure });
    assert.equal(receipt.reason, "helperProcess");
    assert.equal(receipt.helper.exitCode, failure.status);
    assert.equal(receipt.helper.signal, failure.signal ?? null);
    assert.equal(
      receipt.helper.errorCode,
      failure.error
        ? ["ENOENT", "ETIMEDOUT"].includes(failure.error.code)
          ? failure.error.code
          : "OTHER"
        : null,
    );
    assert.equal(JSON.stringify(receipt).includes("secret"), false);
    for (const stage of [0, 1, 2]) {
      let index = 0;
      const checks = serviceChecks("desktop", "probe", () => {
        const current = index++;
        return current === stage
          ? failure
          : output(current === 1 ? { admitted: false } : healthy);
      });
      assert.equal(checks.passed, false);
      assert.equal(
        checks.reason,
        ["trusted-before", "untrusted-process", "trusted-after"][stage],
      );
      assert.equal(checks.steps[stage].signal, failure.signal ?? null);
      assert.equal(JSON.stringify(checks).includes("secret"), false);
    }
  }
});
