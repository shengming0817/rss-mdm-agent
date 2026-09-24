// Run on an administrator-prepared lab machine; no standalone negative PASS.
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync, mkdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
const digest = (file) =>
  createHash("sha256").update(readFileSync(file)).digest("hex");

export function verifiedCandidate(policy, hash = digest) {
  const validArtifact = (artifact) =>
    artifact &&
    typeof artifact.path === "string" &&
    resolve(artifact.path) === artifact.path &&
    /^[a-f0-9]{64}$/.test(artifact.sha256 ?? "") &&
    hash(artifact.path) === artifact.sha256;
  if (
    policy?.version !== 1 ||
    !validArtifact(policy.client) ||
    !validArtifact(policy.probe)
  )
    return undefined;
  return { executable: policy.client.path, negative: policy.probe.path };
}
export function serviceChecks(executable, negative, execute = spawnSync) {
  const steps = [];
  const call = (name, program, args) => {
    const result = execute(program, args, {
      encoding: "utf8",
      timeout: 10000,
      windowsHide: true,
    });
    let response;
    try {
      response = JSON.parse(result.stdout);
    } catch {}
    steps.push({
      name,
      exitCode: result.status,
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
  if (!connected(before)) return { passed: false, steps };
  const denied = call("untrusted-process", negative, []);
  const after = call("trusted-after", executable, ["--service-probe"]);
  return {
    passed:
      denied?.admitted === false &&
      connected(after) &&
      JSON.stringify(before.status) === JSON.stringify(after.status),
    steps,
  };
}
if (
  process.argv[1] &&
  pathToFileURL(resolve(process.argv[1])).href === import.meta.url
) {
  const policyPath =
    process.platform === "win32"
      ? resolve(
          process.env.ProgramData ?? "C:\\ProgramData",
          "RSS MDM Agent/service/policy.json",
        )
      : "/Library/Application Support/RSS MDM Agent/service/policy.json";
  const policy = JSON.parse(readFileSync(policyPath, "utf8"));
  const candidate = verifiedCandidate(policy);
  const checks = candidate
    ? serviceChecks(candidate.executable, candidate.negative)
    : { passed: false, steps: [] };
  const passed = checks.passed && !!verifiedCandidate(policy);
  mkdirSync(".local-ci-runs", { recursive: true });
  writeFileSync(
    ".local-ci-runs/service-platform.json",
    JSON.stringify(
      {
        platform: process.platform,
        arch: process.arch,
        policyPath,
        artifactSha256: policy.client?.sha256,
        probeSha256: policy.probe?.sha256,
        at: new Date().toISOString(),
        status: passed
          ? "passed-controlled-query-and-negative-fixture"
          : "failed",
        steps: checks.steps,
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
      },
      null,
      2,
    ),
  );
  if (!passed)
    throw new Error(
      "Platform query/negative fixture failed; see combined receipt.",
    );
}
