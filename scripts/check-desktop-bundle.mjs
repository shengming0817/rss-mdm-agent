// Actual production .app startup; lifecycle behavior is exercised separately via its shared owner.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { existsSync, mkdtempSync, realpathSync, rmSync } from "node:fs";
import { join } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { run, verifyRuntimeIntegrity } from "./ai-host-artifacts.mjs";

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
      "--debug",
      "--config",
      "src-tauri/tauri.bundle.conf.json",
      "--config",
      '{"build":{"beforeBuildCommand":""}}',
    ],
    root,
  );
  const bundle = join(
    root,
    "target/debug/bundle/macos/RSS MDM Agent.app/Contents",
  );
  verifyRuntimeIntegrity(
    join(bundle, "Resources/ai-host-runtime"),
    runtimeTreeSha256,
  );
  // A short, private home also keeps the macOS Unix socket path within sockaddr_un.
  const isolatedHome = realpathSync(mkdtempSync("/tmp/rss-b-"));
  const data = join(
    isolatedHome,
    "Library/Application Support/com.rss.mdmagent/s1",
  );
  const env = {
    ...process.env,
    HOME: isolatedHome,
    CFFIXED_USER_HOME: isolatedHome,
  };
  delete env.RSS_AI_HOST_RUNTIME;
  delete env.CODEX_HOME;
  const child = spawn(join(bundle, "MacOS/rss-mdm-desktop"), [], {
    cwd: isolatedHome,
    env,
    detached: true,
    stdio: "ignore",
  });
  const exited = once(child, "exit");
  // Observe spawn errors immediately, including during readiness polling.
  let failure;
  exited.catch((error) => {
    failure = error;
  });
  try {
    const deadline = Date.now() + 30000;
    while (!existsSync(join(data, "ai.sock")) && Date.now() < deadline) {
      if (failure) throw failure;
      assert.equal(
        child.exitCode,
        null,
        "production bundle exited before Host readiness",
      );
      assert.equal(child.signalCode, null);
      await delay(100);
    }
    assert.ok(
      existsSync(join(data, "ai.sock")),
      "bundled Host must create its private socket",
    );
    assert.ok(
      existsSync(join(data, "execution.sqlite")),
      "production execution owner must start",
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
    rmSync(isolatedHome, { recursive: true, force: true });
  }
}
