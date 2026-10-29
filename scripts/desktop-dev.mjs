// ref: Node.js lib/child_process.js@v24.14.1 (owned preparation and signal lifecycle)
import { runDesktop, runPreparation } from "./desktop-dev-process.mjs";
import {
  desktopBuildEnvironment,
  organizationBuildInput,
} from "./desktop-organization.mjs";
import { mkdirSync, rmSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
const root = fileURLToPath(new URL("../", import.meta.url));
const args = process.argv.slice(2);
const fixture = args.includes("--fixture");
if (
  args.filter((value) => value === "--fixture").length > 1 ||
  args.some((value) => value.startsWith("--fixture="))
)
  throw new Error("use exactly one --fixture flag");
const tauriArgs = args.filter((value) => value !== "--fixture");
const lock = join(root, ".cache/desktop-dev.lock");
let stage = "assembly selection",
  locked = false,
  preparationCleanupConfirmed = true;
async function prepare(command, args, environment) {
  preparationCleanupConfirmed = false;
  const result = await runPreparation(command, args, root, environment);
  preparationCleanupConfirmed = result.cleanup.confirmed;
  if (result.code !== 0) {
    process.exitCode = result.code;
    throw new Error(
      result.cancellation ? "preparation cancelled" : "preparation failed",
    );
  }
}
function mark(value) {
  stage = value;
  console.log(`[desktop dev] ${value}`);
}
try {
  let buildEnv = { ...process.env };
  delete buildEnv[organizationBuildInput];
  delete buildEnv.VITE_RSS_ASSEMBLY;
  buildEnv.RSS_DESKTOP_ASSEMBLY = fixture ? "fixture" : "production";
  let directory;
  mkdirSync(join(root, ".cache"), { recursive: true });
  mkdirSync(lock);
  locked = true;
  if (fixture) {
    buildEnv.VITE_RSS_ASSEMBLY = "fixture";
    delete buildEnv.RSS_AI_HOST_RUNTIME;
    mark("fixture dependency preparation (no production service or AI Host)");
    for (const preparation of [
      ["build:ai-access"],
      ["--filter", "@rss-mdm-agent/ui", "build"],
    ]) {
      await prepare("pnpm", preparation, buildEnv);
    }
    tauriArgs.unshift(
      "--config",
      JSON.stringify({ build: { devUrl: "http://127.0.0.1:1420/?desktop" } }),
    );
    const featureIndex = tauriArgs.indexOf("--features");
    if (featureIndex >= 0) tauriArgs[featureIndex + 1] += ",dev-fixture";
    else tauriArgs.unshift("--features", "dev-fixture");
  } else {
    mark("backend configuration");
    buildEnv = desktopBuildEnvironment(root, buildEnv);
    const override = process.env.RSS_AI_HOST_RUNTIME;
    mark(
      override === undefined
        ? "AI Host preparation"
        : "AI Host override validation",
    );
    if (override !== undefined && !override.trim())
      throw new Error(
        "RSS_AI_HOST_RUNTIME must be a non-empty runtime directory",
      );
    directory =
      override === undefined
        ? join(root, ".local-ci-runs/ai-host-dev-runtime")
        : resolve(override);
    const preparationEnv = { ...buildEnv };
    delete preparationEnv[organizationBuildInput];
    await prepare(
      process.execPath,
      [
        join(root, "scripts/desktop-dev-runtime.mjs"),
        "--prepare",
        root,
        directory,
        override === undefined ? "build" : "override",
      ],
      preparationEnv,
    );
  }
  rmSync(lock, { recursive: true });
  locked = false;
  mark(
    `Tauri build/start (${fixture ? "fixture" : "production"}); window remains open until explicit quit`,
  );
  const result = await runDesktop(root, directory, tauriArgs, buildEnv);
  process.exitCode = result.code;
  console.log(
    `[desktop dev] ${stage} ${process.exitCode >= 128 ? "cancelled" : "finished"}: ${process.exitCode}`,
  );
} catch (error) {
  console.error(
    `[desktop dev] ${stage} failed: ${error.message}. Check dependencies with pnpm install --frozen-lockfile, then rerun pnpm dev.`,
  );
  process.exitCode ||= 1;
} finally {
  if (locked && preparationCleanupConfirmed)
    rmSync(lock, { recursive: true, force: true });
  else if (locked)
    console.error(
      "[desktop dev] preparation lock retained: process group cleanup unconfirmed; verify owned processes have exited before removing .cache/desktop-dev.lock",
    );
}
