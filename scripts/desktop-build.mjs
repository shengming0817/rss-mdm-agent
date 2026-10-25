import { fileURLToPath } from "node:url";
import { run } from "./ai-host-artifacts.mjs";
import { desktopBuildEnvironment } from "./desktop-organization.mjs";
const native = process.argv.includes("--native-acceptance");
const buildEnv = native
  ? {
      ...process.env,
      RSS_BUILD_MDM_ORGANIZATION: JSON.stringify({
        origin: "https://mdm.native-acceptance.test",
        tenant: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
        label: "Native acceptance",
      }),
    }
  : desktopBuildEnvironment(fileURLToPath(new URL("../", import.meta.url)));
if (
  !(
    (process.platform === "darwin" && process.arch === "arm64") ||
    (process.platform === "win32" && ["x64", "arm64"].includes(process.arch))
  )
)
  throw new Error(
    "Desktop candidate supports macOS arm64 and Windows amd64/arm64 only.",
  );
run(
  "pnpm",
  [
    "exec",
    "tauri",
    "build",
    ...(native
      ? [
          "--debug",
          "--no-bundle",
          "--features",
          "native-e2e",
          "--config",
          JSON.stringify({ build: { beforeBuildCommand: "" } }),
        ]
      : process.platform === "win32"
        ? ["--no-bundle"]
        : []),
    "--config",
    process.platform === "win32"
      ? "src-tauri/tauri.runtime.windows.conf.json"
      : "src-tauri/tauri.bundle.conf.json",
  ],
  fileURLToPath(new URL("../apps/desktop", import.meta.url)),
  buildEnv,
);

if (process.platform === "darwin" && !native) {
  const { join } = await import("node:path");
  const { mkdirSync } = await import("node:fs");
  const { execFileSync } = await import("node:child_process");
  const { cargoTargetDir } = await import("./cargo-target.mjs");
  const { assemblePasskeyHost } = await import("./build-macos-passkey.mjs");
  const root = fileURLToPath(new URL("../", import.meta.url));
  const app = join(
    cargoTargetDir(root),
    "release/bundle/macos/RSS MDM Agent.app",
  );
  const identity =
    process.env.RSS_MACOS_APP_SIGNING_IDENTITY ??
    process.env.RSS_MACOS_DEVELOPMENT_SIGNING_IDENTITY ??
    "-";
  const output = join(root, ".local-ci-runs/macos-passkey");
  mkdirSync(output, { recursive: true });
  const entitlements = assemblePasskeyHost({
    app,
    output: join(output, "host-entitlements.plist"),
    identity,
    release: true,
  });
  execFileSync(
    "/usr/bin/codesign",
    [
      "--force",
      "--sign",
      identity,
      "--entitlements",
      entitlements,
      ...(identity === "-" ? [] : ["--options", "runtime"]),
      app,
    ],
    { stdio: "inherit" },
  );
  execFileSync("/usr/bin/codesign", ["--verify", "--strict", app], {
    stdio: "inherit",
  });
}
