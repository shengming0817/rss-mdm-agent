// Run on an administrator-prepared lab machine; no standalone negative PASS.
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync, mkdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { sourceState } from "./source-state.mjs";
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
  if (!process.argv[2] || !process.argv[3])
    throw new Error("Pass installed desktop and untrusted probe executables.");
  const executable = resolve(process.argv[2]),
    negative = resolve(process.argv[3]);
  const hash = (file) =>
    createHash("sha256").update(readFileSync(file)).digest("hex");
  const desktopHash = hash(executable),
    probeHash = hash(negative);
  const checks = serviceChecks(executable, negative);
  const passed =
    checks.passed &&
    desktopHash === hash(executable) &&
    probeHash === hash(negative);
  mkdirSync(".local-ci-runs", { recursive: true });
  writeFileSync(
    ".local-ci-runs/service-platform.json",
    JSON.stringify(
      {
        source: sourceState(process.cwd()),
        platform: process.platform,
        arch: process.arch,
        artifactSha256: desktopHash,
        probeSha256: probeHash,
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
