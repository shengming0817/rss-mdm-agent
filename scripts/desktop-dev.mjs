// ref: Node.js lib/child_process.js@v24.14.1 (spawn and signal lifecycle)
import { runDesktop } from "./desktop-dev-process.mjs";
import {
  desktopBuildEnvironment,
  organizationBuildInput,
} from "./desktop-organization.mjs";
import { mkdirSync, rmSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  ensureDevelopmentRuntime,
  verifyDevelopmentRuntime,
} from "./desktop-dev-runtime.mjs";
const root = fileURLToPath(new URL("../", import.meta.url));
let stage = "backend configuration",
  locked = false;
const lock = join(root, ".cache/desktop-dev.lock");
let interrupted = false;
const stopPreparation = () => {
  interrupted = true;
};
const preparationSignals = ["SIGINT", "SIGTERM", "SIGHUP"];
for (const signal of preparationSignals) process.on(signal, stopPreparation);
const releaseSignals = () => {
  for (const signal of preparationSignals) process.off(signal, stopPreparation);
};
try {
  const buildEnv = desktopBuildEnvironment(root);
  // Host preparation must not inherit a stale desktop-only build input.
  delete process.env[organizationBuildInput];
  stage = "AI Host preparation";
  mkdirSync(join(root, ".cache"), { recursive: true });
  try {
    mkdirSync(lock);
    locked = true;
  } catch {
    throw new Error(
      "another pnpm dev preparation is active; if interrupted, remove .cache/desktop-dev.lock and retry",
    );
  }
  const override = process.env.RSS_AI_HOST_RUNTIME;
  let directory;
  if (override !== undefined) {
    stage = "AI Host override validation";
    if (!override.trim())
      throw new Error(
        "RSS_AI_HOST_RUNTIME must be a non-empty runtime directory",
      );
    directory = resolve(override);
    verifyDevelopmentRuntime(root, directory);
  } else
    directory = ensureDevelopmentRuntime(
      root,
      join(root, ".local-ci-runs/ai-host-dev-runtime"),
    );
  // Let a group signal observed during synchronous preparation settle before launch.
  await new Promise(setImmediate);
  if (interrupted) throw new Error("preparation cancelled");
  rmSync(lock, { recursive: true });
  locked = false;
  releaseSignals();
  stage = "Tauri startup";
  process.exitCode = await runDesktop(
    root,
    directory,
    process.argv.slice(2),
    buildEnv,
  );
} catch (error) {
  console.error(
    `[desktop dev] ${stage} failed: ${error.message}. Check dependencies with pnpm install --frozen-lockfile, then rerun pnpm dev.`,
  );
  process.exitCode = 1;
} finally {
  releaseSignals();
  if (locked) rmSync(lock, { recursive: true, force: true });
}
