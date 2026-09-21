import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  writeFileSync,
  rmSync,
  copyFileSync,
  chmodSync,
  existsSync,
} from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { tmpdir } from "node:os";
import { createHash } from "node:crypto";
import {
  packHost,
  installArtifacts,
  run,
  runtimeArtifact,
  runtimeTreeSha256 as hashRuntimeTree,
} from "./ai-host-artifacts.mjs";
import { sourceState, sameCommittedSource } from "./source-state.mjs";
const root = fileURLToPath(new URL("../", import.meta.url)),
  start = sourceState(root);
const { version, target, sha256, sqlite } = runtimeArtifact(
  root,
  `${process.platform}-${process.arch}`,
);
if (process.platform !== "darwin" || process.arch !== "arm64")
  throw new Error("This runtime artifact is verified only on macOS arm64");
const directory = join(root, ".local-ci-runs/ai-host-runtime"),
  cache = join(root, ".cache/ai-host"),
  archive = join(cache, `node-v${version}-${target}.tar.gz`),
  scratch = mkdtempSync(join(tmpdir(), "rss-host-node-"));
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
let behaviorPassed = false,
  failure,
  deploymentLockSha256,
  artifacts = [];
try {
  mkdirSync(cache, { recursive: true });
  if (!existsSync(archive) || hash(readFileSync(archive)) !== sha256) {
    const response = await fetch(
      `https://nodejs.org/download/release/v${version}/node-v${version}-${target}.tar.gz`,
      { signal: AbortSignal.timeout(60000) },
    );
    if (!response.ok) throw new Error("Node runtime download failed");
    const bytes = Buffer.from(await response.arrayBuffer());
    if (hash(bytes) !== sha256)
      throw new Error("Node runtime checksum mismatch");
    writeFileSync(archive, bytes);
  }
  run("pnpm", ["build:ai-host"], root);
  rmSync(directory, { recursive: true, force: true });
  mkdirSync(directory, { recursive: true });
  artifacts = packHost(root, directory, true);
  deploymentLockSha256 = installArtifacts(root, directory, true);
  run("/usr/bin/tar", ["-xzf", archive, "-C", scratch], root);
  mkdirSync(join(directory, "bin"));
  copyFileSync(
    join(scratch, `node-v${version}-${target}/bin/node`),
    join(directory, "bin/node"),
  );
  chmodSync(join(directory, "bin/node"), 0o755);
  copyFileSync(
    join(scratch, `node-v${version}-${target}/LICENSE`),
    join(directory, "NODE-LICENSE"),
  );
  writeFileSync(
    join(directory, "bin/rss-ai-host"),
    '#!/bin/sh\nbase=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)\nexec "$base/bin/node" "$base/node_modules/@rss-mdm-agent/ai-host-app/dist/cli.js" "$@"\n',
  );
  chmodSync(join(directory, "bin/rss-ai-host"), 0o755);
  run(
    join(directory, "bin/node"),
    [
      "--eval",
      `if(process.versions.node!=='${version}'||process.versions.sqlite!=='${sqlite}')process.exit(1)`,
    ],
    directory,
  );
  run(join(directory, "bin/rss-ai-host"), ["--help"], directory);
  run(
    join(directory, "bin/node"),
    [
      join(root, "scripts/verify-ai-host-runtime.mjs"),
      join(directory, "bin/rss-ai-host"),
    ],
    directory,
  );
  behaviorPassed = true;
} catch (error) {
  failure = String(error);
  process.exitCode = 1;
} finally {
  rmSync(scratch, { recursive: true, force: true });
  const end = sourceState(root),
    sourceUnchanged = sameCommittedSource(start, end);
  let deliverable = behaviorPassed && sourceUnchanged,
    runtimeTreeSha256;
  if (deliverable) {
    try {
      runtimeTreeSha256 = hashRuntimeTree(directory);
    } catch (error) {
      failure = String(error);
      deliverable = false;
    }
  }
  if (!deliverable) process.exitCode = 1;
  mkdirSync(directory, { recursive: true });
  writeFileSync(
    join(directory, "manifest.json"),
    JSON.stringify(
      {
        status: deliverable ? "passed" : "failed",
        desktopProtocol: 1,
        contractVersion: 5,
        behaviorPassed,
        source: { start, end },
        node: { version, target, archiveSha256: sha256 },
        artifacts,
        lockSha256: hash(readFileSync(join(root, "pnpm-lock.yaml"))),
        deploymentLockSha256,
        runtimeTreeSha256,
        verification: {
          platform: process.platform,
          arch: process.arch,
          externalModel: false,
          desktopIntegration: false,
          bundledCliLifecycle: behaviorPassed,
        },
        failure,
      },
      null,
      2,
    ),
  );
  if (failure) console.error(failure);
  if (!sourceUnchanged)
    console.error("Runtime artifact requires clean committed source");
}
