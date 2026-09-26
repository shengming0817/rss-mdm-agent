// The checkout-built native loader owns installed policy and artifact authorization.
import { constants } from "node:os";
import { spawnSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { resolve, posix, win32 } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

function executeProcess(execute, program, args, options) {
  try {
    return execute(program, args, options);
  } catch (error) {
    return { status: null, error };
  }
}

function processResult(name, result) {
  const codes = [
    "ENOENT",
    "EACCES",
    "EPERM",
    "ETIMEDOUT",
    "ENOBUFS",
    "ENOMEM",
    "E2BIG",
  ];
  return {
    name,
    exitCode: Number.isInteger(result.status) ? result.status : null,
    signal: Object.hasOwn(constants.signals, result.signal)
      ? result.signal
      : null,
    errorCode: result.error
      ? codes.includes(result.error.code)
        ? result.error.code
        : "OTHER"
      : null,
  };
}

export function serviceChecks(executable, negative, execute = spawnSync) {
  const steps = [];
  const call = (name, program, args) => {
    const result = executeProcess(execute, program, args, {
      encoding: "utf8",
      timeout: 10000,
      windowsHide: true,
    });
    let response;
    try {
      response = JSON.parse(result.stdout);
    } catch {}
    steps.push({
      ...processResult(name, result),
      pid: result.pid,
      stdout: result.stdout,
      stderr: result.stderr,
      response,
    });
    return result.status === 0 && !result.error && !result.signal
      ? response
      : undefined;
  };
  const connected = (view) =>
    view?.phase === "connected" &&
    view.status?.capability === "statusOnly" &&
    view.status?.version === 1 &&
    typeof view.status.installation === "string" &&
    typeof view.status.build === "string";
  const before = call("trusted-before", executable, ["--service-probe"]);
  if (!connected(before))
    return { passed: false, reason: "trusted-before", steps };
  const denied = call("untrusted-process", negative, []);
  const after = call("trusted-after", executable, ["--service-probe"]);
  const reason =
    denied?.admitted !== false
      ? "untrusted-process"
      : !connected(after) ||
          JSON.stringify(before.status) !== JSON.stringify(after.status)
        ? "trusted-after"
        : null;
  return { passed: reason === null, reason, steps };
}

function writeReceipt(receipt) {
  mkdirSync(".local-ci-runs", { recursive: true });
  writeFileSync(
    ".local-ci-runs/service-platform.json",
    JSON.stringify(receipt, null, 2),
  );
}

export function verifyLocalService({
  platform = process.platform,
  execute = spawnSync,
  writeReceipt: save = writeReceipt,
} = {}) {
  const receipt = {
    platform,
    arch: process.arch,
    at: new Date().toISOString(),
    status: "failed",
    policyPath: null,
    policyVersion: null,
    helperVersion: null,
    permissionCheck: "unavailable",
    reason: "helperProcess",
    helper: null,
    steps: [],
    notCovered: [
      "cross-user",
      "fake-server",
      "worker",
      "tamper",
      "replay",
      "revocation",
      "restart",
      "real-desktop-codex",
    ],
  };
  // Invalidate an earlier PASS before any helper or candidate can run.
  save(receipt);
  try {
    if (!["win32", "darwin"].includes(platform))
      throw Error("unsupported platform");
    const helper = fileURLToPath(
      new URL(
        `../target/release/rss-local-service${platform === "win32" ? ".exe" : ""}`,
        import.meta.url,
      ),
    );
    const result = executeProcess(
      execute,
      helper,
      ["--verification-candidate"],
      {
        encoding: "utf8",
        timeout: 10000,
        maxBuffer: 65536,
        windowsHide: true,
      },
    );
    receipt.helper = processResult("native-helper", result);
    if (result.error || result.signal || result.status === null)
      throw Error("native helper failed");
    receipt.reason = "helperOutput";
    const value = JSON.parse(result.stdout);
    const absolute = (path) =>
      typeof path === "string" &&
      (platform === "win32" ? win32 : posix).isAbsolute(path);
    const fields = [
      "version",
      "phase",
      "policyPath",
      "policyVersion",
      "helperVersion",
      "permissionCheck",
      "executable",
      "negative",
      "reason",
    ];
    if (
      !value ||
      typeof value !== "object" ||
      Array.isArray(value) ||
      Object.keys(value).some((key) => !fields.includes(key)) ||
      value.version !== 1 ||
      typeof value.helperVersion !== "string" ||
      !value.helperVersion ||
      !(value.policyPath === null || absolute(value.policyPath)) ||
      !["unavailable", "failed", "passed"].includes(value.permissionCheck) ||
      !(value.policyVersion === null || value.policyVersion === 1) ||
      !["verified", "rejected"].includes(value.phase) ||
      (value.phase === "rejected" &&
        (!["policyPath", "protection", "policyValidation"].includes(
          value.reason,
        ) ||
          "executable" in value ||
          "negative" in value))
    )
      throw Error("invalid native projection");
    receipt.reason =
      value.phase === "rejected"
        ? value.reason
        : result.status !== 0
          ? "helperProcess"
          : "helperOutput";
    receipt.policyPath = value.policyPath;
    receipt.helperVersion = value.helperVersion;
    receipt.policyVersion = value.policyVersion;
    receipt.permissionCheck = value.permissionCheck;
    if (
      result.status !== 0 ||
      result.error ||
      result.signal ||
      value.phase !== "verified" ||
      value.policyVersion !== 1 ||
      value.permissionCheck !== "passed" ||
      !absolute(value.policyPath) ||
      !absolute(value.executable) ||
      !absolute(value.negative) ||
      value.reason != null
    )
      throw Error("native policy rejected");
    receipt.reason = "serviceChecks";
    const checks = serviceChecks(value.executable, value.negative, execute);
    receipt.steps = checks.steps;
    receipt.reason = checks.reason;
    if (checks.passed) {
      receipt.status = "passed-controlled-query-and-negative-fixture";
      receipt.reason = null;
    }
  } catch {
    // Closed diagnostics: never project untrusted OS/provider error text.
    receipt.status = "failed";
  }
  save(receipt);
  return receipt;
}

if (
  process.argv[1] &&
  pathToFileURL(resolve(process.argv[1])).href === import.meta.url
) {
  if (
    verifyLocalService().status !==
    "passed-controlled-query-and-negative-fixture"
  ) {
    console.error(
      "Platform query/negative fixture failed; see .local-ci-runs/service-platform.json.",
    );
    process.exitCode = 1;
  }
}
