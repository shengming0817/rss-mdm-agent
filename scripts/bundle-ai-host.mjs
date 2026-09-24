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
  stageWorkerRuntime,
  installArtifacts,
  run,
  runtimeArtifact,
  runtimeTreeSha256 as hashRuntimeTree,
} from "./ai-host-artifacts.mjs";
import { developmentFingerprint } from "./desktop-dev-runtime.mjs";
const development = process.argv.includes("--development");
const root = fileURLToPath(new URL("../", import.meta.url)),
  start = development ? developmentFingerprint(root) : undefined;
const { version, target, sha256, sqlite } = runtimeArtifact(
  root,
  `${process.platform}-${process.arch}`,
);
const windows = process.platform === "win32";
const suffix = windows ? ".exe" : "";
const extension = windows ? "zip" : "tar.gz";
const directory = join(
    root,
    development
      ? ".local-ci-runs/ai-host-dev-runtime"
      : ".local-ci-runs/ai-host-runtime",
  ),
  cache = join(root, ".cache/ai-host"),
  archive = join(cache, `node-v${version}-${target}.${extension}`),
  scratch = mkdtempSync(join(tmpdir(), "rss-host-node-"));
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
let stage = "Node archive verification",
  behaviorPassed = false,
  failure;
try {
  mkdirSync(cache, { recursive: true });
  if (!existsSync(archive) || hash(readFileSync(archive)) !== sha256) {
    const response = await fetch(
      `https://nodejs.org/download/release/v${version}/node-v${version}-${target}.${extension}`,
      { signal: AbortSignal.timeout(60000) },
    );
    if (!response.ok) throw new Error("Node runtime download failed");
    const bytes = Buffer.from(await response.arrayBuffer());
    if (hash(bytes) !== sha256)
      throw new Error("Node runtime checksum mismatch");
    writeFileSync(archive, bytes);
  }
  stage = "AI Host build";
  run("pnpm", ["build:ai-host"], root);
  rmSync(directory, { recursive: true, force: true });
  mkdirSync(directory, { recursive: true });
  stage = "AI Host dependency packaging";
  packHost(root, directory);
  installArtifacts(root, directory);
  stage = "Node runtime extraction";
  run(
    windows
      ? join(process.env.SystemRoot, "System32", "tar.exe")
      : "/usr/bin/tar",
    ["-xf", archive, "-C", scratch],
    root,
  );
  mkdirSync(join(directory, "bin"));
  copyFileSync(
    join(
      scratch,
      `node-v${version}-${target}`,
      windows ? "node.exe" : "bin/node",
    ),
    join(directory, "bin/node" + suffix),
  );
  chmodSync(join(directory, "bin/node" + suffix), 0o755);
  copyFileSync(
    join(scratch, `node-v${version}-${target}/LICENSE`),
    join(directory, "NODE-LICENSE"),
  );
  stageWorkerRuntime(root, directory, join(directory, "bin/node" + suffix));
  stage = "runtime validation";
  run(
    join(directory, "bin/node" + suffix),
    [
      "--eval",
      `if(process.versions.node!=='${version}'||process.versions.sqlite!=='${sqlite}')process.exit(1)`,
    ],
    directory,
  );
  run(
    join(directory, "bin/node" + suffix),
    [
      join(directory, "node_modules/@rss-mdm-agent/ai-host-app/dist/cli.js"),
      "--help",
    ],
    directory,
  );
  run(
    join(directory, "bin/node" + suffix),
    [join(root, "scripts/verify-ai-host-runtime.mjs"), directory],
    directory,
  );
  behaviorPassed = true;
} catch (error) {
  failure = `${stage}: ${error}`;
  process.exitCode = 1;
} finally {
  rmSync(scratch, { recursive: true, force: true });
  const end = development ? developmentFingerprint(root) : undefined,
    sourceUnchanged = !development || start === end;
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
        desktopProtocol: JSON.parse(
          readFileSync(
            join(root, "packages/ai-contract/schema/runtime.schema.json"),
            "utf8",
          ),
        ).$defs.HostHealth.properties.protocol.const,
        contractVersion: 5,
        behaviorPassed,
        ...(development
          ? { kind: "development", developmentFingerprint: end }
          : { kind: "release" }),
        node: { version, target, archiveSha256: sha256 },
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
    console.error("AI Host source changed during build; rerun pnpm dev");
}
