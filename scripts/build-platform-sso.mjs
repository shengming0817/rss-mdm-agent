// Native subartifact builder shared by the development shell and the existing installer.
// ref: Apple Foundation.tbd NSExtensionMain; AuthenticationServices headers (SDK 26.4).
import { execFileSync, spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { cargoTargetDir } from "./cargo-target.mjs";
const root = fileURLToPath(new URL("../", import.meta.url));
export function buildPlatformSso({
  output,
  identity = "-",
  release = false,
  verifyAbi = false,
}) {
  if (process.platform !== "darwin" || process.arch !== "arm64")
    throw Error("PSSO development requires native macOS arm64");
  const run = (command, args, options = {}) =>
    execFileSync(command, args, { cwd: root, timeout: 1800000, ...options });
  const sdk = run("xcrun", ["--show-sdk-version"], { encoding: "utf8" }).trim();
  if (sdk !== "26.4") throw Error("PSSO candidate requires SDK 26.4");
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
      "platform-sso-extension",
      ...(release ? ["--release"] : []),
    ],
    { stdio: "inherit" },
  );
  const contents = join(output, "Contents");
  mkdirSync(join(contents, "MacOS"), { recursive: true });
  const binary = join(contents, "MacOS/RSSPlatformSSO");
  const nativeArguments = [
    "clang",
    "-arch",
    "arm64",
    "-mmacosx-version-min=26.4",
    "-fobjc-arc",
    "-fblocks",
    "-fapplication-extension",
    join(root, "apps/platform-sso-extension/native/Extension.m"),
    join(
      cargoTargetDir(root),
      release ? "release" : "debug",
      "libplatform_sso_extension.a",
    ),
    "-framework",
    "AppKit",
    "-framework",
    "AuthenticationServices",
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
      join(root, "apps/platform-sso-extension/resources/Info.plist"),
      "utf8",
    ).replaceAll("__VERSION__", version),
  );
  run("/usr/bin/plutil", ["-lint", join(contents, "Info.plist")]);
  run("/usr/bin/codesign", [
    "--force",
    "--sign",
    identity,
    "--entitlements",
    join(root, "apps/platform-sso-extension/resources/Entitlements.plist"),
    ...(identity === "-" ? [] : ["--options", "runtime"]),
    output,
  ]);
  run("/usr/bin/codesign", ["--verify", "--strict", output]);
  const display = spawnSync(
    "/usr/bin/codesign",
    ["-d", "--verbose=4", output],
    { encoding: "utf8", timeout: 20000 },
  );
  if (display.status !== 0) throw Error("PSSO signing inspection failed");
  const teamId =
    /^TeamIdentifier=([A-Z0-9]{10})$/m.exec(display.stderr)?.[1] ?? null;
  if (identity !== "-" && !teamId) throw Error("Apple signing team required");
  return { binary, teamId, sdk, systemAccepted: false };
}
export function containerEntitlements(app, output) {
  const display = spawnSync(
    "/usr/bin/codesign",
    ["-d", "--entitlements", ":-", app],
    { encoding: "utf8", timeout: 20000 },
  );
  let existing = {};
  if (display.stdout.trim())
    existing = JSON.parse(
      execFileSync(
        "/usr/bin/plutil",
        ["-convert", "json", "-o", "-", "--", "-"],
        { input: display.stdout, encoding: "utf8" },
      ),
    );
  existing["com.apple.developer.associated-domains.mdm-managed"] = true;
  existing["com.apple.developer.associated-domains"] ??= [];
  writeFileSync(output, JSON.stringify(existing));
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
      "Configure development identity through RSS_MACOS_DEVELOPMENT_SIGNING_IDENTITY",
    );
  const output = join(root, ".local-ci-runs/platform-sso/RSSPlatformSSO.appex");
  const result = buildPlatformSso({
    output,
    identity: process.env.RSS_MACOS_DEVELOPMENT_SIGNING_IDENTITY ?? "-",
    verifyAbi: process.argv.includes("--verify-abi"),
  });
  console.log(JSON.stringify({ output, ...result }));
}
