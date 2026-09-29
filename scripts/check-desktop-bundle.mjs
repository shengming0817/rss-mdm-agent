// Actual production .app startup; lifecycle behavior is exercised separately via its shared owner.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  realpathSync,
  rmSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { setTimeout as delay } from "node:timers/promises";
import { run, verifyRuntimeIntegrity } from "./ai-host-artifacts.mjs";
import { cargoTargetDir } from "./cargo-target.mjs";
import { sourceEvidence, sha256 } from "./native-evidence.mjs";

export async function checkDesktopBundle(root, runtimeTreeSha256) {
  run("pnpm", ["stage:desktop-runtime"], root);
  run(
    "pnpm",
    [
      "--filter",
      "@rss-mdm-agent/desktop",
      "exec",
      "tauri",
      "build",
      "--config",
      "src-tauri/tauri.bundle.conf.json",
      "--config",
      '{"build":{"beforeBuildCommand":""}}',
    ],
    root,
  );
  const bundle = join(
    cargoTargetDir(root),
    "release/bundle/macos/RSS MDM Agent.app/Contents",
  );
  verifyRuntimeIntegrity(
    join(bundle, "Resources/ai-host-runtime"),
    runtimeTreeSha256,
  );
  // A short, isolated data root keeps Unix sockets within sockaddr_un.
  const isolatedData = realpathSync(mkdtempSync("/tmp/rss-b-"));
  const data = isolatedData;
  const env = { ...process.env };
  delete env.RSS_AI_HOST_RUNTIME;
  delete env.CODEX_HOME;
  const child = spawn(
    join(bundle, "MacOS/rss-mdm-desktop"),
    ["--test-data-dir", data],
    {
      cwd: isolatedData,
      env,
      detached: true,
      stdio: ["ignore", "ignore", "pipe"],
    },
  );
  let status,
    buffer = "";
  child.stderr.setEncoding("utf8");
  child.stderr.on("data", (chunk) => {
    buffer = (buffer + chunk).slice(-65536);
    let newline;
    while ((newline = buffer.indexOf("\n")) >= 0) {
      const line = buffer.slice(0, newline);
      buffer = buffer.slice(newline + 1);
      if (line.startsWith("RSS_AI_HOST_STATUS ")) {
        try {
          status = JSON.parse(line.slice("RSS_AI_HOST_STATUS ".length));
        } catch {}
      }
    }
  });
  const exited = once(child, "exit");
  // Observe spawn errors immediately, including during readiness polling.
  let failure;
  exited.catch((error) => {
    failure = error;
  });
  try {
    const deadline = Date.now() + 30000;
    while (!status && Date.now() < deadline) {
      if (failure) throw failure;
      assert.equal(
        child.exitCode,
        null,
        "production bundle exited before Host readiness",
      );
      assert.equal(child.signalCode, null);
      await delay(100);
    }
    assert.equal(
      status?.phase,
      "ready",
      "native owner must complete the Host health handshake",
    );
    assert.equal(status?.source, "bundled_resource");
    assert.ok(
      existsSync(join(data, "users.json")),
      "native registry must be ready before user selection",
    );
    assert.equal(
      existsSync(join(data, "execution.sqlite")),
      true,
      "one device execution service starts independently of user selection",
    );
    assert.ok(
      existsSync(join(data, "ai.sqlite")),
      "bundled Host must open its own SQLite store",
    );
    await delay(1000);
    assert.equal(child.exitCode, null);
    assert.equal(child.signalCode, null);
    return {
      productionEntrypoint: true,
      buildProfile: "release",
      nativeDriver: false,
      resourceOverride: false,
      bundledHostReady: true,
      runtimeTreeSha256,
      startupOnly: true,
      cleanup: "isolated process group termination",
    };
  } finally {
    // This smoke does not claim graceful quit: native acceptance covers the shared shutdown owner.
    if (child.pid) {
      try {
        process.kill(-child.pid, "SIGTERM");
      } catch (error) {
        if (error.code !== "ESRCH") throw error;
      }
      await Promise.race([exited.catch(() => {}), delay(5000)]);
      try {
        process.kill(-child.pid, "SIGKILL");
      } catch (error) {
        if (error.code !== "ESRCH") throw error;
      }
      await exited.catch(() => {});
    }
    rmSync(isolatedData, { recursive: true, force: true });
  }
}

if (
  process.argv[1] &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const root = fileURLToPath(new URL("../", import.meta.url));

  const report = join(root, ".local-ci-runs/desktop-bundle.json");
  mkdirSync(join(root, ".local-ci-runs"), { recursive: true });
  const evidence = {
    status: "running",
    mode: "release-bundle-startup",
    credentials: "none",
  };
  writeFileSync(report, JSON.stringify(evidence, null, 2));
  try {
    evidence.source = sourceEvidence(root);
    const manifestBytes = readFileSync(
      join(root, ".local-ci-runs/ai-host-runtime/manifest.json"),
    );
    evidence.runtimeManifestSha256 = sha256(manifestBytes);
    const manifest = JSON.parse(manifestBytes);
    assert.equal(manifest.status, "passed", "Build the runtime first");
    run("pnpm", ["build"], root);
    evidence.result = await checkDesktopBundle(
      root,
      manifest.runtimeTreeSha256,
    );
    assert.deepEqual(sourceEvidence(root), evidence.source);
    evidence.status = "passed";
  } catch {
    evidence.status = "failed";
    evidence.failure = "bundle_startup_failed";
    process.exitCode = 1;
  } finally {
    writeFileSync(report, JSON.stringify(evidence, null, 2));
  }
}
