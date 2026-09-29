// Actual production release .app startup; daily native acceptance owns UI/lifecycle proof.
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
  renameSync,
} from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { setTimeout as delay } from "node:timers/promises";
import { verifyRuntimeIntegrity } from "./ai-host-artifacts.mjs";
import { cargoTargetDir } from "./cargo-target.mjs";
import { sourceEvidence, sha256 } from "./native-evidence.mjs";

// Every spawned command owns a separate process group, including build descendants.
function owned(command, args, options, signal) {
  signal.throwIfAborted();
  const child = spawn(command, args, { ...options, detached: true });
  const exited = once(child, "exit");
  let failure;
  exited.catch((error) => {
    failure = error;
  });
  const kill = (name) => {
    if (!child.pid) return;
    try {
      process.kill(-child.pid, name);
    } catch (error) {
      if (!["ESRCH", "EPERM"].includes(error.code)) throw error;
    }
  };
  const alive = () => {
    if (!child.pid) return false;
    try {
      process.kill(-child.pid, 0);
      return true;
    } catch (error) {
      if (error.code === "EPERM") return true;
      if (error.code !== "ESRCH") throw error;
      return false;
    }
  };
  const abort = () => {
    kill("SIGTERM");
  };
  signal.addEventListener("abort", abort, { once: true });
  if (signal.aborted) abort();
  return {
    child,
    exited,
    check() {
      signal.throwIfAborted();
      if (failure) throw failure;
    },
    async close() {
      signal.removeEventListener("abort", abort);
      kill("SIGTERM");
      await Promise.race([exited.catch(() => {}), delay(5000)]);
      if (alive()) kill("SIGKILL");
      const deadline = Date.now() + 5000;
      while (alive() && Date.now() < deadline) await delay(50);
      assert.equal(alive(), false, "owned bundle process group did not exit");
    },
  };
}
async function command(root, args, signal) {
  const owner = owned("pnpm", args, { cwd: root, stdio: "inherit" }, signal);
  try {
    const [code] = await owner.exited;
    owner.check();
    assert.equal(code, 0, "bundle build command failed");
  } finally {
    await owner.close();
  }
}
export async function checkDesktopBundle(
  root,
  runtimeTreeSha256,
  { signal, stage },
) {
  stage("stage-runtime");
  await command(root, ["stage:desktop-runtime"], signal);
  stage("release-build");
  await command(
    root,
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
    signal,
  );
  const bundle = join(
    cargoTargetDir(root),
    "release/bundle/macos/RSS MDM Agent.app/Contents",
  );
  stage("bundled-integrity");
  verifyRuntimeIntegrity(
    join(bundle, "Resources/ai-host-runtime"),
    runtimeTreeSha256,
  );
  const data = realpathSync(mkdtempSync("/tmp/rss-b-"));
  const env = { ...process.env };
  delete env.RSS_AI_HOST_RUNTIME;
  delete env.CODEX_HOME;
  let owner;
  try {
    stage("host-readiness");
    owner = owned(
      join(bundle, "MacOS/rss-mdm-desktop"),
      ["--test-data-dir", data],
      { cwd: data, env, stdio: ["ignore", "ignore", "pipe"] },
      signal,
    );
    let status,
      buffer = "";
    owner.child.stderr.setEncoding("utf8");
    owner.child.stderr.on("data", (chunk) => {
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
    const deadline = Date.now() + 30000;
    while (!status && Date.now() < deadline) {
      owner.check();
      assert.equal(
        owner.child.exitCode,
        null,
        "bundle exited before readiness",
      );
      assert.equal(owner.child.signalCode, null);
      await delay(100);
    }
    owner.check();
    assert.equal(status?.phase, "ready");
    assert.equal(status?.source, "bundled_resource");
    stage("startup-state");
    for (const file of ["users.json", "execution.sqlite", "ai.sqlite"])
      assert.ok(
        existsSync(join(data, file)),
        "production owner storage must be ready",
      );
    await delay(1000, undefined, { signal });
    owner.check();
    assert.equal(owner.child.exitCode, null);
    assert.equal(owner.child.signalCode, null);
    return {
      productionEntrypoint: true,
      buildProfile: "release",
      nativeDriver: false,
      resourceOverride: false,
      bundledHostReady: true,
      runtimeTreeSha256,
      mainBinarySha256: sha256(
        readFileSync(join(bundle, "MacOS/rss-mdm-desktop")),
      ),
      startupOnly: true,
      cleanup: "isolated process group termination and absence check",
    };
  } finally {
    // This smoke proves startup; graceful application quit belongs to native acceptance.
    try {
      await owner?.close();
    } catch (error) {
      stage("cleanup");
      throw error;
    } finally {
      rmSync(data, { recursive: true, force: true });
    }
  }
}
if (
  process.argv[1] &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const root = fileURLToPath(new URL("../", import.meta.url));
  const report = join(root, ".local-ci-runs/desktop-bundle.json");
  mkdirSync(join(root, ".local-ci-runs"), { recursive: true });
  const cancellation = new AbortController();
  for (const signal of ["SIGINT", "SIGTERM"])
    process.on(signal, () =>
      cancellation.abort(new Error("bundle check cancelled")),
    );
  const evidence = {
    status: "running",
    mode: "release-bundle-startup",
    credentials: "none",
    stage: "preflight",
  };
  const write = () => {
    writeFileSync(report + ".tmp", JSON.stringify(evidence, null, 2));
    renameSync(report + ".tmp", report);
  };
  const stage = (value) => {
    evidence.stage = value;
    write();
  };
  write();
  try {
    evidence.source = sourceEvidence(root);
    stage("runtime-manifest");
    const manifestBytes = readFileSync(
      join(root, ".local-ci-runs/ai-host-runtime/manifest.json"),
    );
    evidence.runtimeManifestSha256 = sha256(manifestBytes);
    const manifest = JSON.parse(manifestBytes);
    assert.equal(manifest.status, "passed", "Build the runtime first");
    stage("frontend-build");
    await command(root, ["build"], cancellation.signal);
    evidence.result = await checkDesktopBundle(
      root,
      manifest.runtimeTreeSha256,
      { signal: cancellation.signal, stage },
    );
    stage("source-integrity");
    assert.deepEqual(sourceEvidence(root), evidence.source);
    cancellation.signal.throwIfAborted();
    evidence.status = "passed";
  } catch {
    evidence.status = cancellation.signal.aborted ? "cancelled" : "failed";
    evidence.failure = {
      stage: evidence.stage,
      code:
        evidence.status === "cancelled"
          ? "bundle_check_cancelled"
          : "bundle_check_failed",
    };
    console.error(
      `Release bundle check ${evidence.status} at ${evidence.stage}`,
    );
    process.exitCode = 1;
  } finally {
    write();
  }
}
