// Native subartifact builder shared by the development shell and the existing installer.
// ref: Apple Foundation.tbd NSExtensionMain; AuthenticationServices headers (SDK 26.4).
import { execFileSync, spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { cargoTargetDir } from "./cargo-target.mjs";
const root = fileURLToPath(new URL("../", import.meta.url));
export function buildMacosPasskey({
  output,
  identity = "-",
  release = false,
  verifyAbi = false,
}) {
  if (process.platform !== "darwin" || process.arch !== "arm64")
    throw Error("Passkey development requires native macOS arm64");
  const run = (command, args, options = {}) =>
    execFileSync(command, args, { cwd: root, timeout: 1800000, ...options });
  const sdk = run("xcrun", ["--show-sdk-version"], { encoding: "utf8" }).trim();
  if (sdk !== "26.4") throw Error("Passkey candidate requires SDK 26.4");
  const version = JSON.parse(
    readFileSync(join(root, "package.json"), "utf8"),
  ).version;
  if (!/^\d+(\.\d+)*$/.test(version)) throw Error("Invalid bundle version");
  run(
    "cargo",
    [
      "build",
      "--locked",
      "-p",
      "macos-passkey-extension",
      ...(release ? ["--release"] : []),
    ],
    { stdio: "inherit" },
  );
  const contents = join(output, "Contents");
  mkdirSync(join(contents, "MacOS"), { recursive: true });
  const binary = join(contents, "MacOS/RSSPasskeyProvider");
  const nativeArguments = [
    "clang",
    "-arch",
    "arm64",
    "-mmacosx-version-min=15.0",
    "-fobjc-arc",
    "-fblocks",
    "-fapplication-extension",
    join(root, "apps/macos-passkey-extension/native/Extension.m"),
    join(
      cargoTargetDir(root),
      release ? "release" : "debug",
      "libmacos_passkey_extension.a",
    ),
    "-framework",
    "AppKit",
    "-framework",
    "AuthenticationServices",
    "-framework",
    "LocalAuthentication",
    "-framework",
    "Security",
    "-framework",
    "SystemConfiguration",
    "-framework",
    "CoreGraphics",
    "-framework",
    "CoreFoundation",
    "-framework",
    "Foundation",
    "-liconv",
    "-lresolv",
    "-Wl,-e,_NSExtensionMain",
    "-o",
    binary,
  ];
  run("xcrun", nativeArguments, { stdio: "inherit" });
  if (verifyAbi) {
    const probe = join(dirname(output), "BridgeTest");
    const args = nativeArguments
      .filter((arg) => arg !== "-Wl,-e,_NSExtensionMain")
      .map((arg) =>
        arg === binary
          ? probe
          : arg.endsWith("native/Extension.m")
            ? arg.replace("Extension.m", "BridgeTest.m")
            : arg,
      );
    run("xcrun", args, { stdio: "inherit" });
    run(probe, [], { stdio: "inherit", timeout: 20000 });
  }
  writeFileSync(
    join(contents, "Info.plist"),
    readFileSync(
      join(root, "apps/macos-passkey-extension/resources/Info.plist"),
      "utf8",
    ).replaceAll("__VERSION__", version),
  );
  run("/usr/bin/plutil", ["-lint", join(contents, "Info.plist")]);
  run("/usr/bin/codesign", [
    "--force",
    "--sign",
    identity,
    "--entitlements",
    join(root, "apps/macos-passkey-extension/resources/Entitlements.plist"),
    ...(identity === "-" ? [] : ["--options", "runtime"]),
    ...(process.env.RSS_MACOS_NOTARY_PROFILE ? ["--timestamp"] : []),
    output,
  ]);
  run("/usr/bin/codesign", ["--verify", "--strict", output]);
  const display = spawnSync(
    "/usr/bin/codesign",
    ["-d", "--verbose=4", output],
    { encoding: "utf8", timeout: 20000 },
  );
  if (display.status !== 0) throw Error("Passkey signing inspection failed");
  return { binary, sdk, systemAccepted: false };
}
// Single host configuration used by the standalone app build and the installer.
export function passkeyEntitlements(existing) {
  const group = "group.com.rss.mdmagent.personal-passkeys";
  return {
    ...existing,
    "com.apple.developer.authentication-services.autofill-credential-provider": true,
    "com.apple.security.application-groups": [
      ...new Set([
        ...(existing["com.apple.security.application-groups"] ?? []),
        group,
      ]),
    ],
    "keychain-access-groups": [
      ...new Set([...(existing["keychain-access-groups"] ?? []), group]),
    ],
  };
}
export function assemblePasskeyHost({
  app,
  output,
  identity = "-",
  release = true,
}) {
  buildMacosPasskey({
    output: join(app, "Contents/PlugIns/RSSPasskeyProvider.appex"),
    identity,
    release,
  });
  return configurePasskeyHost(app, output);
}
export function configurePasskeyHost(app, output) {
  const infoPath = join(app, "Contents/Info.plist");
  const info = JSON.parse(
    execFileSync("/usr/bin/plutil", ["-convert", "json", "-o", "-", infoPath], {
      encoding: "utf8",
    }),
  );
  info.ASCredentialProviderExtensionCapabilities = {
    ...info.ASCredentialProviderExtensionCapabilities,
    ProvidesPasskeys: true,
  };
  writeFileSync(infoPath, JSON.stringify(info));
  execFileSync("/usr/bin/plutil", ["-convert", "xml1", infoPath]);
  const displayed = spawnSync(
    "/usr/bin/codesign",
    ["-d", "--entitlements", ":-", app],
    { encoding: "utf8", timeout: 20000 },
  );
  const existing = displayed.stdout.trim()
    ? JSON.parse(
        execFileSync(
          "/usr/bin/plutil",
          ["-convert", "json", "-o", "-", "--", "-"],
          { input: displayed.stdout, encoding: "utf8" },
        ),
      )
    : {};
  writeFileSync(output, JSON.stringify(passkeyEntitlements(existing)));
  execFileSync("/usr/bin/plutil", ["-convert", "xml1", output]);
  return output;
}
if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(resolve(process.argv[1])).href
) {
  if (
    process.argv.length > 3 ||
    (process.argv.length === 3 && process.argv[2] !== "--verify-abi")
  )
    throw Error(
      "Configure signing through RSS_MACOS_APP_SIGNING_IDENTITY or RSS_MACOS_DEVELOPMENT_SIGNING_IDENTITY",
    );
  const output = join(
    root,
    ".local-ci-runs/macos-passkey/RSSPasskeyProvider.appex",
  );
  const result = buildMacosPasskey({
    output,
    identity:
      process.env.RSS_MACOS_APP_SIGNING_IDENTITY ??
      process.env.RSS_MACOS_DEVELOPMENT_SIGNING_IDENTITY ??
      "-",
    verifyAbi: process.argv.includes("--verify-abi"),
  });
  console.log(JSON.stringify({ output, ...result }));
}
