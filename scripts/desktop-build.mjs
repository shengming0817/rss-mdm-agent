import { fileURLToPath } from "node:url";
import { run } from "./ai-host-artifacts.mjs";
if (
  !(
    (process.platform === "darwin" && process.arch === "arm64") ||
    (process.platform === "win32" && process.arch === "x64")
  )
)
  throw new Error(
    "Desktop candidate supports macOS arm64 and Windows x64 only.",
  );
run(
  "pnpm",
  [
    "exec",
    "tauri",
    "build",
    "--config",
    process.platform === "win32"
      ? "src-tauri/tauri.windows.conf.json"
      : "src-tauri/tauri.bundle.conf.json",
  ],
  fileURLToPath(new URL("../apps/desktop", import.meta.url)),
);

if (process.platform === "win32") await import("./stage-windows-desktop.mjs");
