import { fileURLToPath } from "node:url";
import { run, verifyRuntimeIntegrity } from "./ai-host-artifacts.mjs";
import { sourceEvidence, sha256 } from "./native-evidence.mjs";
import { verifyFrontendClosure } from "./frontend-closure.mjs";
import { join } from "node:path";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import assert from "node:assert/strict";
import { desktopBuildEnvironment } from "./desktop-organization.mjs";
const native = process.argv.includes("--native-acceptance");
const root = fileURLToPath(new URL("../", import.meta.url));
const receipt = join(root, ".local-ci-runs/desktop-release-static.json");
if (!native) rmSync(receipt, { force: true });
const fixedSource = native ? undefined : sourceEvidence(root);
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
  const { execFileSync } = await import("node:child_process");
  const { cargoTargetDir } = await import("./cargo-target.mjs");
  const { assemblePasskeyHost, appleSigned } = await import(
    "./build-macos-passkey.mjs"
  );
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
  execFileSync("/usr/bin/codesign", ["--force", "--sign", identity, app], {
    stdio: "inherit",
  });
  const entitlements = assemblePasskeyHost({
    app,
    output: join(output, "host-entitlements.plist"),
    identity,
    release: true,
    enabled: appleSigned(app),
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

if (!native) {
  const { cargoTargetDir } = await import("./cargo-target.mjs");
  const desktop = verifyFrontendClosure(
    join(root, "apps/desktop/dist"),
    fixedSource,
  );
  const ui = verifyFrontendClosure(join(root, "packages/ui/dist"), fixedSource);
  const app =
    process.platform === "darwin"
      ? join(
          cargoTargetDir(root),
          "release/bundle/macos/RSS MDM Agent.app/Contents",
        )
      : undefined;
  const binary = app
    ? join(app, "MacOS/rss-mdm-desktop")
    : join(cargoTargetDir(root), "release/rss-mdm-desktop.exe");
  const runtime = app
    ? join(app, "Resources/ai-host-runtime")
    : join(root, "apps/desktop/src-tauri/resources/ai-host-runtime");
  const runtimeManifest = readFileSync(join(runtime, "manifest.json"));
  const manifest = JSON.parse(runtimeManifest);
  assert.equal(manifest.status, "passed");
  assert.equal(manifest.kind, "release");
  assert.equal(manifest.verification.platform, process.platform);
  assert.equal(manifest.verification.arch, process.arch);
  verifyRuntimeIntegrity(runtime, manifest.runtimeTreeSha256);
  assert.deepEqual(
    sourceEvidence(root),
    fixedSource,
    "source changed during release build",
  );
  mkdirSync(join(root, ".local-ci-runs"), { recursive: true });
  writeFileSync(
    receipt,
    JSON.stringify(
      {
        status: "passed",
        kind: "static-release-package",
        source: fixedSource,
        environment: { platform: process.platform, arch: process.arch },
        binarySha256: sha256(readFileSync(binary)),
        runtimeManifestSha256: sha256(runtimeManifest),
        runtimeTreeSha256: manifest.runtimeTreeSha256,
        frontend: Object.fromEntries(
          [desktop, ui].map((report) => [
            report.owner,
            { modules: report.modules.length, files: report.files },
          ]),
        ),
        macosCodeSignature:
          process.platform === "darwin"
            ? "codesign --verify --strict passed"
            : "not_applicable",
        nativeAcceptance: "not_verified",
        applicationStarted: false,
        windowsPackaging:
          process.platform === "win32"
            ? "executable and staged runtime only"
            : "not_verified",
      },
      null,
      2,
    ) + "\n",
  );
  console.log(
    "Fixed release package and frontend closure verified statically; application was not started",
  );
}
