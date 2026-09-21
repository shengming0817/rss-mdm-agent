import { cpSync, mkdirSync, readFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { verifyRuntimeIntegrity } from "./ai-host-artifacts.mjs";
import { sourceState } from "./source-state.mjs";
const root = fileURLToPath(new URL("../", import.meta.url));
const source = join(root, ".local-ci-runs/ai-host-runtime");
const manifest = JSON.parse(
  readFileSync(join(source, "manifest.json"), "utf8"),
);
const state = sourceState(root);
if (
  manifest.status !== "passed" ||
  manifest.kind !== "release" ||
  !state.clean ||
  manifest.source.end.head !== state.head ||
  process.platform !== "darwin" ||
  process.arch !== "arm64"
)
  throw new Error(
    "A verified artifact from this committed macOS arm64 source is required",
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
])
  cpSync(join(source, name), join(destination, name), {
    recursive: true,
    verbatimSymlinks: true,
  });
verifyRuntimeIntegrity(destination, manifest.runtimeTreeSha256);
console.log("Desktop runtime staged from the verified local candidate");
