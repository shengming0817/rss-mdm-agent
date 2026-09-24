import { cpSync, mkdirSync, readFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { verifyRuntimeIntegrity } from "./ai-host-artifacts.mjs";
const root = fileURLToPath(new URL("../", import.meta.url));
const source = join(root, ".local-ci-runs/ai-host-runtime");
const manifest = JSON.parse(
  readFileSync(join(source, "manifest.json"), "utf8"),
);
if (
  manifest.status !== "passed" ||
  manifest.kind !== "release" ||
  manifest.verification.platform !== process.platform ||
  manifest.verification.arch !== process.arch
)
  throw new Error(
    "A successfully built runtime for the current platform is required",
  );
verifyRuntimeIntegrity(source, manifest.runtimeTreeSha256);
const destination = join(
  root,
  "apps/desktop/src-tauri/resources/ai-host-runtime",
);
rmSync(destination, { recursive: true, force: true });
mkdirSync(destination, { recursive: true });
for (const name of [
  "bin",
  "node_modules",
  "manifest.json",
  "NODE-LICENSE",
  "package.json",
  "pnpm-lock.yaml",
  "worker-manifest.json",
])
  cpSync(join(source, name), join(destination, name), {
    recursive: true,
    verbatimSymlinks: true,
  });
verifyRuntimeIntegrity(destination, manifest.runtimeTreeSha256);
console.log("Desktop runtime staged from the verified local candidate");
