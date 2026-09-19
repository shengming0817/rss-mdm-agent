// Real macOS WebView acceptance; uses the fixed runtime artifact and existing user login.
import { spawn } from "node:child_process";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";
import { sourceState, sameCommittedSource } from "./source-state.mjs";
import { run } from "./ai-host-artifacts.mjs";
const root = fileURLToPath(new URL("../", import.meta.url)),
  start = sourceState(root);
if (process.platform !== "darwin" || process.arch !== "arm64")
  throw new Error("acceptance requires macOS arm64");
const artifact = join(root, ".local-ci-runs/ai-host-runtime"),
  manifest = JSON.parse(readFileSync(join(artifact, "manifest.json"), "utf8"));
if (
  manifest.status !== "passed" ||
  !sameCommittedSource(start, manifest.source.end)
)
  throw new Error("same-source fixed artifact required");
const directory = realpathSync(mkdtempSync(join(tmpdir(), "rss-desktop-"))),
  report = join(directory, "result.json");
let behavior, failure;
try {
  run("pnpm", ["build"], root);
  run(
    "cargo",
    [
      "build",
      "-p",
      "rss-mdm-desktop",
      "--example",
      "desktop-acceptance",
      "--locked",
    ],
    root,
  );
  await new Promise((resolve, reject) => {
    const child = spawn(
      join(root, "target/debug/examples/desktop-acceptance"),
      [directory, artifact, report],
      { cwd: root, stdio: ["ignore", "ignore", "ignore"] },
    );
    const timer = setTimeout(() => {
      child.kill("SIGKILL");
      reject(new Error("native process timeout"));
    }, 270000);
    child.once("error", (error) => {
      clearTimeout(timer);
      reject(error);
    });
    child.once("exit", (code, signal) => {
      clearTimeout(timer);
      if (code !== 0 || signal !== null)
        reject(new Error("native process did not exit cleanly"));
      else resolve();
    });
  });
  behavior = JSON.parse(readFileSync(report, "utf8"));
  if (behavior.step !== "passed")
    throw new Error(`native acceptance failed: ${behavior.stage}`);
} catch (error) {
  failure = String(error);
  process.exitCode = 1;
} finally {
  const end = sourceState(root),
    passed = behavior?.step === "passed" && sameCommittedSource(start, end);
  mkdirSync(join(root, ".local-ci-runs"), { recursive: true });
  writeFileSync(
    join(root, ".local-ci-runs/desktop-native.json"),
    JSON.stringify(
      {
        status: passed ? "passed" : "failed",
        source: { start, end },
        platform: process.platform,
        arch: process.arch,
        lockSha256: createHash("sha256")
          .update(readFileSync(join(root, "pnpm-lock.yaml")))
          .digest("hex"),
        runtimeManifestSha256: createHash("sha256")
          .update(readFileSync(join(artifact, "manifest.json")))
          .digest("hex"),
        authentication: "existing_user_config",
        modelFixture: false,
        executor: "S1 deterministic test runner",
        behavior,
        failure,
      },
      null,
      2,
    ),
  );
  rmSync(directory, { recursive: true, force: true });
  if (!passed) process.exitCode = 1;
  if (failure) console.error(failure);
}
