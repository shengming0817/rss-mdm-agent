import { createHash } from "node:crypto";
import { lstatSync, readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import {
  run,
  runtimeArtifact,
  verifyRuntimeIntegrity,
} from "./ai-host-artifacts.mjs";

// Hash source bytes, including uncommitted/untracked files, never Git HEAD or build output.
export function developmentFingerprint(root) {
  const hash = createHash("sha256");
  function visit(name) {
    const path = join(root, name),
      stat = lstatSync(path);
    if (stat.isDirectory()) {
      for (const child of readdirSync(path).sort()) {
        if (!["node_modules", "dist", ".git"].includes(child))
          visit(`${name}/${child}`);
      }
    } else if (stat.isFile()) {
      hash.update(`${name}\0${stat.size}\0`);
      hash.update(readFileSync(path));
    } else throw new Error(`Unsupported development source: ${name}`);
  }
  for (const name of [
    "package.json",
    "pnpm-lock.yaml",
    "pnpm-workspace.yaml",
    "scripts/bundle-ai-host.mjs",
    "scripts/ai-host-artifacts.mjs",
    "scripts/desktop-dev-runtime.mjs",
    "scripts/verify-ai-host-runtime.mjs",
    "apps/ai-host",
    "packages/ai-host",
    "packages/ai-contract",
    "packages/ai-store-sqlite",
    "packages/ai-access",
    "packages/ai-client",
    "packages/ai-adapters/claude",
    "packages/ai-adapters/codex",
    "packages/ai-adapters/deepseek",
  ])
    visit(name);
  return hash.digest("hex");
}

export function verifyDevelopmentRuntime(root, directory) {
  const manifest = JSON.parse(
    readFileSync(join(directory, "manifest.json"), "utf8"),
  );
  const node = runtimeArtifact(root, `${process.platform}-${process.arch}`);
  const protocol = JSON.parse(
    readFileSync(
      join(root, "packages/ai-contract/schema/runtime.schema.json"),
      "utf8",
    ),
  ).$defs.HostHealth.properties.protocol.const;
  if (
    manifest.status !== "passed" ||
    manifest.desktopProtocol !== protocol ||
    manifest.contractVersion !== 5 ||
    manifest.node?.version !== node.version ||
    manifest.node?.archiveSha256 !== node.sha256 ||
    manifest.node?.target !== node.target ||
    manifest.verification?.platform !== process.platform ||
    manifest.verification?.arch !== process.arch
  )
    throw new Error(
      "runtime manifest is incompatible; rebuild the runtime with the pinned Node version",
    );
  verifyRuntimeIntegrity(directory, manifest.runtimeTreeSha256);
  run(
    join(directory, "bin/node"),
    [
      "--eval",
      `if(process.versions.node!==${JSON.stringify(node.version)}||process.versions.sqlite!==${JSON.stringify(node.sqlite)})process.exit(1)`,
    ],
    directory,
  );
  run(
    join(directory, "bin/node"),
    [
      join(root, "scripts/verify-ai-host-runtime.mjs"),
      join(directory, "bin/rss-ai-host"),
    ],
    directory,
  );
}

export function ensureDevelopmentRuntime(
  root,
  directory,
  {
    build = () =>
      run(
        process.execPath,
        [join(root, "scripts/bundle-ai-host.mjs"), "--development"],
        root,
      ),
    verify = verifyDevelopmentRuntime,
  } = {},
) {
  const fingerprint = developmentFingerprint(root);
  let manifest;
  try {
    manifest = JSON.parse(
      readFileSync(join(directory, "manifest.json"), "utf8"),
    );
  } catch {
    /* Missing/incomplete cache is rebuilt. */
  }
  if (
    manifest?.kind !== "development" ||
    manifest.developmentFingerprint !== fingerprint ||
    manifest.status === "failed"
  )
    build();
  verify(root, directory);
  if (developmentFingerprint(root) !== fingerprint)
    throw new Error(
      "AI Host source changed during preparation; rerun pnpm dev",
    );
  return directory;
}
