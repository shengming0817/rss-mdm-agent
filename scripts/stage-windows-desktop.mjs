import {
  cpSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";
import { verifyRuntimeIntegrity } from "./ai-host-artifacts.mjs";
if (process.platform !== "win32" || process.arch !== "x64")
  throw new Error("Windows x64 staging requires its native build.");
const root = fileURLToPath(new URL("../", import.meta.url));

const runtime = join(root, "apps/desktop/src-tauri/resources/ai-host-runtime");
const manifest = JSON.parse(
  readFileSync(join(runtime, "manifest.json"), "utf8"),
);
if (
  manifest.status !== "passed" ||
  manifest.verification.platform !== "win32" ||
  manifest.verification.arch !== "x64"
)
  throw new Error("Desktop runtime must match the current platform.");
verifyRuntimeIntegrity(runtime, manifest.runtimeTreeSha256);
const output = join(root, ".local-ci-runs/windows-lab-desktop");
rmSync(output, { recursive: true, force: true });
mkdirSync(output, { recursive: true });
const executable = join(output, "rss-mdm-desktop.exe");
cpSync(join(root, "target/release/rss-mdm-desktop.exe"), executable);
cpSync(runtime, join(output, "ai-host-runtime"), { recursive: true });
verifyRuntimeIntegrity(
  join(output, "ai-host-runtime"),
  manifest.runtimeTreeSha256,
);
writeFileSync(
  join(output, "desktop-manifest.json"),
  JSON.stringify(
    {
      platform: "win32",
      arch: "x64",
      executableSha256: createHash("sha256")
        .update(readFileSync(executable))
        .digest("hex"),
      runtimeTreeSha256: manifest.runtimeTreeSha256,
    },
    null,
    2,
  ),
);
console.log(
  "Windows administrator-install candidate: .local-ci-runs/windows-lab-desktop",
);
