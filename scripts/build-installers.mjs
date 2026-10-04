// Fixed organization packages contain only public trust. Device enrollment is a separate operation.
// ref: Apple pkgbuild(1); NSIS 3.11 Docs/Chapter4.html (per-machine execution level).
import { execFileSync, spawnSync } from "node:child_process";
import {
  cpSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { createHash, randomUUID, X509Certificate } from "node:crypto";
import { cargoTargetDir } from "./cargo-target.mjs";
import { runtimeTreeSha256 } from "./ai-host-artifacts.mjs";
import { sourceEvidence } from "./native-evidence.mjs";
const root = fileURLToPath(new URL("../", import.meta.url));
const args = process.argv.slice(2);
const input = args.indexOf("--organization");
if (
  input < 0 ||
  !args[input + 1] ||
  args.some((arg, i) => i !== input && i !== input + 1 && arg !== "--release")
)
  throw Error(
    "Use --organization /absolute/public-organization.json [--release]",
  );
const windows = process.platform === "win32";
if (
  !(windows && ["x64", "arm64"].includes(process.arch)) &&
  !(process.platform === "darwin" && process.arch === "arm64")
)
  throw Error("Build on native macOS arm64 or Windows amd64/arm64");
const release = args.includes("--release");
const organization = JSON.parse(readFileSync(resolve(args[input + 1]), "utf8"));
const allowed = ["origin", "tenant", "label", "signing_keys", "ca_pem"];
if (Object.keys(organization).some((key) => !allowed.includes(key)))
  throw Error("Organization input accepts public trust fields only");
if (
  new URL(organization.origin).origin !== organization.origin ||
  !organization.origin.startsWith("https://") ||
  !/^[0-9a-f-]{36}$/.test(organization.tenant) ||
  !organization.label ||
  !Object.keys(organization.signing_keys ?? {}).length
)
  throw Error("Invalid organization identity or signing keys");
for (const pem of [organization.ca_pem].filter(Boolean)) {
  if (
    !/^\s*-----BEGIN CERTIFICATE-----[A-Za-z0-9+/=\s]+-----END CERTIFICATE-----\s*$/.test(
      pem,
    ) ||
    !new X509Certificate(pem).ca
  )
    throw Error("One public CA certificate required");
}
if (!organization.ca_pem)
  throw Error("A protected HTTPS bootstrap CA is required");
for (const key of Object.values(organization.signing_keys))
  if (Buffer.from(key, "base64url").length !== 32)
    throw Error("Ed25519 public key required");
const env = {
  ...process.env,
  RSS_BUILD_MDM_ORGANIZATION: JSON.stringify({
    origin: organization.origin,
    tenant: organization.tenant,
    label: organization.label,
    ...(organization.ca_pem ? { ca_pem: organization.ca_pem } : {}),
  }),
};
// The outer build runner retains the worktree/target locks for this entire package build.
// pnpm descendants do not inherit their descriptors and must not claim a borrowed lease.
delete env._AGENT_BUILD_LEASE;
const run = (command, argv, cwd = root) =>
  execFileSync(command, argv, { cwd, env, stdio: "inherit", timeout: 1800000 });
const pnpm = (argv, cwd = root) =>
  windows
    ? run(
        process.execPath,
        [
          process.env.npm_execpath ??
            (() => {
              throw Error(
                "Use pnpm exec node scripts/build-installers.mjs on Windows",
              );
            })(),
          ...argv,
        ],
        cwd,
      )
    : run("pnpm", argv, cwd);
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
const version = JSON.parse(readFileSync(join(root, "package.json"))).version;
const target = windows
  ? `windows-${process.arch === "x64" ? "amd64" : "arm64"}`
  : "macos-arm64";
const source = sourceEvidence(root);
const id = `${version}-${randomUUID()}`;
const output = join(root, ".local-ci-runs/installers", target, id);
rmSync(output, { recursive: true, force: true });
mkdirSync(output, { recursive: true });
const suffix = windows ? ".exe" : "";
const sign = (file) => {
  if (windows) {
    if (release)
      run(process.env.RSS_SIGNTOOL ?? "signtool.exe", [
        "sign",
        "/sha1",
        process.env.RSS_WINDOWS_SIGNING_THUMBPRINT ??
          (() => {
            throw Error("RSS_WINDOWS_SIGNING_THUMBPRINT required");
          })(),
        "/fd",
        "SHA256",
        "/tr",
        "https://timestamp.digicert.com",
        "/td",
        "SHA256",
        file,
      ]);
  } else {
    run("/usr/bin/codesign", [
      "--force",
      "--sign",
      release
        ? (process.env.RSS_MACOS_APP_SIGNING_IDENTITY ??
          (() => {
            throw Error("RSS_MACOS_APP_SIGNING_IDENTITY required");
          })())
        : "-",
      ...(release
        ? [
            "--options",
            "runtime",
            "--timestamp",
            "--preserve-metadata=entitlements",
          ]
        : []),
      file,
    ]);
  }
};
// Sign native resources before the runtime digest is embedded into DesktopHost.
pnpm(["bundle:ai-host"]);
pnpm(["stage:desktop-runtime"]);
const runtime = join(root, "apps/desktop/src-tauri/resources/ai-host-runtime");
function signNative(directory) {
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const file = join(directory, entry.name);
    if (entry.isDirectory()) signNative(file);
    else if (entry.isFile()) {
      const header = readFileSync(file).subarray(0, 4);
      const native = windows
        ? header.subarray(0, 2).toString() === "MZ"
        : [
            "cffaedfe",
            "cefaedfe",
            "cafebabe",
            "bebafeca",
            "cafebabf",
            "bfbafeca",
          ].includes(header.toString("hex"));
      if (native) sign(file);
    }
  }
}
signNative(runtime);
const workerFile = join(runtime, "worker-manifest.json");
const worker = JSON.parse(readFileSync(workerFile));
worker.node_sha256 = hash(readFileSync(join(runtime, worker.node)));
writeFileSync(workerFile, JSON.stringify(worker) + "\n");
const runtimeManifestFile = join(runtime, "manifest.json");
const runtimeManifest = JSON.parse(readFileSync(runtimeManifestFile));
runtimeManifest.runtimeTreeSha256 = runtimeTreeSha256(runtime);
writeFileSync(
  runtimeManifestFile,
  JSON.stringify(runtimeManifest, null, 2) + "\n",
);
run("cargo", [
  "build",
  "--release",
  "--locked",
  "-p",
  "agent-service",
  "--bin",
  "rss-execution-service",
]);
pnpm(
  [
    "exec",
    "tauri",
    "build",
    "--config",
    JSON.stringify({ build: { beforeBuildCommand: "pnpm -w build" } }),
    "--config",
    windows
      ? "src-tauri/tauri.windows.conf.json"
      : "src-tauri/tauri.bundle.conf.json",
    ...(windows ? ["--no-bundle"] : []),
  ],
  join(root, "apps/desktop"),
);
const payload = join(output, "payload");
mkdirSync(payload, { recursive: true });
const service = join(payload, "rss-execution-service" + suffix);
cpSync(
  join(cargoTargetDir(root), "release/rss-execution-service" + suffix),
  service,
);
sign(service);
let desktop, packagedRuntime;
if (windows) {
  desktop = join(payload, "rss-mdm-desktop.exe");
  cpSync(join(cargoTargetDir(root), "release/rss-mdm-desktop.exe"), desktop);
  packagedRuntime = join(payload, "ai-host-runtime");
  cpSync(runtime, packagedRuntime, { recursive: true });
} else {
  const app = join(payload, "RSS MDM Agent.app");
  cpSync(
    join(cargoTargetDir(root), "release/bundle/macos/RSS MDM Agent.app"),
    app,
    { recursive: true, verbatimSymlinks: true },
  );
  desktop = join(app, "Contents/MacOS/rss-mdm-desktop");
  packagedRuntime = join(app, "Contents/Resources/ai-host-runtime");
  sign(app);
}
if (windows) sign(desktop);
const installed = windows
  ? join(
      process.env.ProgramW6432 ?? process.env.ProgramFiles,
      "RSS MDM Agent/versions",
      id,
    )
  : `/Library/Application Support/RSS MDM Agent/versions/${id}`;
const artifact = (path) => ({
  path: join(installed, path.slice(payload.length + 1)),
  sha256: hash(readFileSync(path)),
  cdhash: null,
});
function image(path) {
  const result = artifact(path);
  if (!windows) {
    const code = spawnSync("/usr/bin/codesign", ["-d", "--verbose=4", path], {
      encoding: "utf8",
      timeout: 20000,
    });
    if (code.status !== 0) throw Error("Signed image required");
    const resultCode = code.stderr;
    result.cdhash =
      /^CDHash=([0-9a-f]{40})$/m.exec(resultCode)?.[1] ??
      (() => {
        throw Error("Code identity missing");
      })();
  }
  return result;
}
const candidate = {
  version: 2,
  product_version: version,
  target,
  root: installed,
  service: image(service),
  desktop: image(desktop),
  runtime_root: join(installed, packagedRuntime.slice(payload.length + 1)),
  runtime_sha256: runtimeTreeSha256(packagedRuntime),
  origin: organization.origin,
  tenant: organization.tenant,
  signing_keys: organization.signing_keys,
  ca_pem: organization.ca_pem ?? null,
  signing_mode: release ? "release" : "development",
};
const candidateBytes = JSON.stringify(candidate, null, 2) + "\n";
writeFileSync(join(payload, "candidate.json"), candidateBytes);
let packagePath;
if (windows) {
  packagePath = join(output, `rss-mdm-agent-${version}-${target}.exe`);
  const template = readFileSync(
    join(root, "scripts/installers/windows.nsi"),
    "utf8",
  );
  // NSIS literals cannot safely represent arbitrary dollar signs or quotes.
  for (const value of [output, payload, installed, id, version])
    if (/[\r\n"$]/.test(value)) throw Error("Unsafe package path");
  const script = template
    .replaceAll("@PAYLOAD@", payload)
    .replaceAll("@OUTPUT@", packagePath)
    .replaceAll("@ID@", id)
    .replaceAll("@VERSION@", version)
    .replaceAll("@SERVICE_SHA256@", candidate.service.sha256)
    .replaceAll("@SERVICE_SIZE@", String(statSync(service).size))
    .replaceAll("@MACHINE@", process.arch === "arm64" ? "0xaa64" : "0x8664");
  const file = join(output, "installer.nsi");
  writeFileSync(file, script);
  run("makensis.exe", [file]);
  sign(packagePath);
} else {
  const scripts = join(output, "scripts");
  mkdirSync(scripts);
  const postinstall = `#!/bin/sh\nset -eu\nprofile="/Library/Application Support/RSS MDM Agent/install-profile.json"\nif [ -f "$profile" ]; then\n  "${installed}/rss-execution-service" --installation apply --candidate "${installed}/candidate.json" --profile "$profile"\nelse\n  "${installed}/rss-execution-service" --installation apply --candidate "${installed}/candidate.json"\nfi\n`;
  writeFileSync(join(scripts, "postinstall"), postinstall, { mode: 0o755 });
  const component = join(output, "component.pkg");
  // A fixed version must never relocate to a discovered development/previous app bundle.
  const components = join(output, "components.plist");
  writeFileSync(
    components,
    `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><array><dict>
<key>RootRelativeBundlePath</key><string>RSS MDM Agent.app</string>
<key>BundleIsRelocatable</key><false/>
<key>BundleIsVersionChecked</key><false/>
<key>BundleHasStrictIdentifier</key><true/>
<key>BundleOverwriteAction</key><string>upgrade</string>
</dict></array></plist>\n`,
  );
  run("/usr/bin/pkgbuild", [
    "--root",
    payload,
    "--component-plist",
    components,
    "--install-location",
    installed,
    "--identifier",
    "com.rss-mdm.agent",
    "--version",
    version,
    "--ownership",
    "recommended",
    "--scripts",
    scripts,
    component,
  ]);
  packagePath = join(output, `rss-mdm-agent-${version}-${target}.pkg`);
  run("/usr/bin/productbuild", [
    "--package",
    component,
    ...(release
      ? [
          "--sign",
          process.env.RSS_MACOS_INSTALLER_SIGNING_IDENTITY ??
            (() => {
              throw Error("RSS_MACOS_INSTALLER_SIGNING_IDENTITY required");
            })(),
        ]
      : []),
    packagePath,
  ]);
}
if (release && !windows) {
  const profile =
    process.env.RSS_MACOS_NOTARY_PROFILE ??
    (() => {
      throw Error("RSS_MACOS_NOTARY_PROFILE keychain reference required");
    })();
  run("/usr/bin/xcrun", [
    "notarytool",
    "submit",
    packagePath,
    "--keychain-profile",
    profile,
    "--wait",
  ]);
  run("/usr/bin/xcrun", ["stapler", "staple", packagePath]);
  run("/usr/bin/xcrun", ["stapler", "validate", packagePath]);
}
const completedSource = sourceEvidence(root);
for (const key of [
  "head",
  "sourceSha256",
  "cargoLockSha256",
  "pnpmLockSha256",
  "node",
  "pnpm",
])
  if (source[key] !== completedSource[key])
    throw Error(
      "Source or toolchain changed during installer build; candidate is not deliverable",
    );
writeFileSync(
  join(output, "package-manifest.json"),
  JSON.stringify(
    {
      version: 1,
      target,
      productVersion: version,
      signingMode: candidate.signing_mode,
      source,
      candidateSha256: hash(candidateBytes),
      packageSha256: hash(readFileSync(packagePath)),
      package: packagePath,
    },
    null,
    2,
  ) + "\n",
);
console.log(`Fixed installer written: ${packagePath}`);
