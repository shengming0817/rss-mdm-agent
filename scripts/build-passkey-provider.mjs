// Windows-only independent component builder. Does not build the enterprise service or desktop.
import { execFileSync } from "node:child_process";
import {
  mkdirSync,
  readFileSync,
  writeFileSync,
  cpSync,
  existsSync,
} from "node:fs";
import { dirname, resolve, join } from "node:path";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";
import {
  identityManifest,
  unsignedPublisher,
  sdk,
} from "./passkey-package.mjs";
const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
if (process.platform !== "win32" || process.arch !== "x64")
  throw Error("Run this builder on target Windows x64; no cross compilation");
const flags = new Set(process.argv.slice(2));
if ([...flags].some((v) => v !== "--release"))
  throw Error("Usage: node scripts/build-passkey-provider.mjs [--release]");
const release = flags.has("--release");
const publisher = release
  ? process.env.RSS_WINDOWS_PUBLISHER
  : unsignedPublisher;
const thumbprint = process.env.RSS_WINDOWS_SIGNING_THUMBPRINT;
if (release && (!publisher || !/^[a-f0-9]{40}$/i.test(thumbprint ?? "")))
  throw Error(
    "Release requires publisher and installed signing certificate thumbprint",
  );
const bin = join(
  process.env["ProgramFiles(x86)"] ?? "C:\\Program Files (x86)",
  "Windows Kits",
  "10",
  "bin",
  sdk,
  "x64",
);
for (const name of ["makeappx.exe", "signtool.exe"])
  if (!existsSync(join(bin, name)))
    throw Error(`Pinned Windows SDK ${sdk} missing: ${name}`);
const run = (cmd, args, env = process.env) =>
  execFileSync(cmd, args, { cwd: root, stdio: "inherit", env });
const output = join(
  root,
  ".local-passkey",
  release ? "release" : "development",
);
const payload = join(output, "payload");
const identity = join(output, "identity");
mkdirSync(payload, { recursive: true });
mkdirSync(identity, { recursive: true });
const version =
  JSON.parse(readFileSync(join(root, "package.json"), "utf8")).version + ".0";
writeFileSync(
  join(identity, "AppxManifest.xml"),
  identityManifest({ version, publisher }),
);
run("cargo", ["build", "--locked", "--release", "-p", "rss-passkey-provider"], {
  ...process.env,
  RSS_PASSKEY_PUBLISHER: publisher,
});
cpSync(
  join(root, "target/release/rss-passkey-provider.exe"),
  join(payload, "rss-passkey-provider.exe"),
);
// Existing application icons are source assets, not a dependency on the desktop runtime.
mkdirSync(join(payload, "Assets"), { recursive: true });
for (const [name, source] of [
  ["StoreLogo.png", "icon.png"],
  ["Logo.png", "icon.png"],
  ["SmallLogo.png", "icon.png"],
])
  cpSync(
    join(root, "apps/desktop/src-tauri/icons", source),
    join(payload, "Assets", name),
  );
const packagePath = join(payload, "identity.msix");
run(join(bin, "makeappx.exe"), [
  "pack",
  "/o",
  "/nv",
  "/d",
  identity,
  "/p",
  packagePath,
]);
const sign = (path) => {
  if (release)
    run(join(bin, "signtool.exe"), [
      "sign",
      "/fd",
      "SHA256",
      "/sha1",
      thumbprint,
      path,
    ]);
};
sign(join(payload, "rss-passkey-provider.exe"));
sign(packagePath);
cpSync(
  join(root, "scripts/installers/passkey-identity.ps1"),
  join(payload, "identity.ps1"),
);
const sha256 = (path) =>
  createHash("sha256").update(readFileSync(path)).digest("hex");
writeFileSync(
  join(payload, "deployment.json"),
  JSON.stringify(
    {
      version,
      publisher,
      release,
      providerSha256: sha256(join(payload, "rss-passkey-provider.exe")),
      identitySha256: sha256(packagePath),
    },
    null,
    2,
  ),
);
const installer = join(output, "rss-passkeys-x64.exe");
const literal = (s) => {
  if (/[\r\n$]/.test(s)) throw Error("Unsupported NSIS path");
  return s.replaceAll('"', '$\\"');
};
const template = readFileSync(
  join(root, "scripts/installers/passkey.nsi"),
  "utf8",
);
writeFileSync(
  join(output, "installer.nsi"),
  template
    .replaceAll("@PAYLOAD@", literal(payload))
    .replaceAll("@OUTPUT@", literal(installer)),
);
run("makensis.exe", [join(output, "installer.nsi")]);
sign(installer);
console.log(
  JSON.stringify({
    installer,
    sha256: sha256(installer),
    platformVerified: false,
    signingMode: release ? "signed" : "development-unsigned",
  }),
);
