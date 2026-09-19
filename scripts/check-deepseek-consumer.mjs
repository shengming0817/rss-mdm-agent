import { spawnSync } from "node:child_process";
import {
  mkdtempSync,
  readFileSync,
  readdirSync,
  writeFileSync,
  rmSync,
  mkdirSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";
import { sourceState, sameCommittedSource } from "./source-state.mjs";
const root = fileURLToPath(new URL("../", import.meta.url)),
  start = sourceState(root),
  directory = mkdtempSync(join(tmpdir(), "rss-deepseek-consumer-"));
let behaviorPassed = false,
  failure,
  artifacts,
  lockSha256;
const hash = (p) => createHash("sha256").update(readFileSync(p)).digest("hex");
function run(command, args, cwd = directory) {
  const result = spawnSync(command, args, {
    cwd,
    stdio: "inherit",
    env: { ...process.env, NODE_PATH: "", PNPM_WORKSPACE_DIR: "" },
  });
  if (result.status !== 0) throw Error(`Consumer command failed: ${command}`);
}
try {
  run("pnpm", ["build:ai-deepseek"], root);
  for (const pkg of ["packages/ai-contract", "packages/ai-adapters/deepseek"])
    run("pnpm", [
      "--dir",
      join(root, pkg),
      "pack",
      "--pack-destination",
      directory,
    ]);
  const archives = readdirSync(directory).filter((f) => f.endsWith(".tgz"));
  const contract = archives.find((f) =>
      f.startsWith("rss-mdm-agent-ai-contract-"),
    ),
    adapter = archives.find((f) =>
      f.startsWith("rss-mdm-agent-ai-adapter-deepseek-"),
    );
  if (!contract || !adapter) throw Error("missing artifacts");
  artifacts = Object.fromEntries(
    archives.map((f) => [f, hash(join(directory, f))]),
  );
  const versions = JSON.parse(
    readFileSync(join(root, "package.json"), "utf8"),
  ).devDependencies;
  writeFileSync(
    join(directory, "package.json"),
    JSON.stringify({
      name: "isolated-deepseek-consumer",
      private: true,
      type: "module",
      dependencies: {
        "@rss-mdm-agent/ai-contract": `file:./${contract}`,
        "@rss-mdm-agent/ai-adapter-deepseek": `file:./${adapter}`,
      },
      devDependencies: {
        typescript: versions.typescript,
        "@types/node": versions["@types/node"],
      },
    }),
  );
  writeFileSync(
    join(directory, "pnpm-workspace.yaml"),
    `packages: []\noverrides: ${JSON.stringify({ "@rss-mdm-agent/ai-contract": `file:./${contract}` })}\n`,
  );
  writeFileSync(
    join(directory, "tsconfig.json"),
    JSON.stringify({
      compilerOptions: {
        target: "ES2022",
        module: "NodeNext",
        moduleResolution: "NodeNext",
        strict: true,
        skipLibCheck: false,
        types: ["node"],
        outDir: "out",
      },
      include: ["consumer.ts"],
    }),
  );
  writeFileSync(
    join(directory, "consumer.ts"),
    readFileSync(join(root, "tests/ai-adapters/deepseek/consumer.ts")),
  );
  run("pnpm", ["install", "--offline"]);
  lockSha256 = hash(join(directory, "pnpm-lock.yaml"));
  run("pnpm", ["exec", "tsc"]);
  run("node", ["out/consumer.js"]);
  const manifest = JSON.parse(
    readFileSync(
      join(
        directory,
        "node_modules/@rss-mdm-agent/ai-adapter-deepseek/package.json",
      ),
      "utf8",
    ),
  );
  if (Object.keys(manifest.exports).join() !== ".")
    throw Error("internal testing interface escaped");
  if (readFileSync(join(directory, "pnpm-lock.yaml"), "utf8").includes(root))
    throw Error("source checkout escaped into artifact consumer");
  behaviorPassed = true;
} catch (error) {
  failure = String(error);
  process.exitCode = 1;
} finally {
  rmSync(directory, { recursive: true, force: true });
  const end = sourceState(root),
    passed = behaviorPassed && sameCommittedSource(start, end);
  if (!passed) process.exitCode = 1;
  mkdirSync(join(root, ".local-ci-runs"), { recursive: true });
  writeFileSync(
    join(root, ".local-ci-runs/deepseek-consumer.json"),
    JSON.stringify(
      {
        status: passed ? "passed" : "failed",
        behaviorPassed,
        source: { start, end },
        artifacts,
        lockSha256,
        platform: process.platform,
        arch: process.arch,
        node: process.version,
        failure,
      },
      null,
      2,
    ),
  );
  if (failure) console.error(failure);
  if (!sameCommittedSource(start, end))
    console.error(
      "DeepSeek consumer: clean committed source required for delivery evidence",
    );
}
