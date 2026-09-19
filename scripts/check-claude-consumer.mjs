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
import { sourceState, sameCommittedSource } from "./source-state.mjs";
const root = fileURLToPath(new URL("../", import.meta.url)),
  start = sourceState(root),
  directory = mkdtempSync(join(tmpdir(), "rss-claude-consumer-"));
let behaviorPassed = false,
  failure;
function run(command, args, cwd = directory) {
  const result = spawnSync(command, args, {
    cwd,
    stdio: "inherit",
    env: { ...process.env, NODE_PATH: "", PNPM_WORKSPACE_DIR: "" },
  });
  if (result.status !== 0)
    throw new Error(`Consumer command failed: ${command}`);
}
try {
  run("pnpm", ["build:ai-claude"], root);
  for (const pkg of ["packages/ai-contract", "packages/ai-adapters/claude"])
    run("pnpm", [
      "--dir",
      join(root, pkg),
      "pack",
      "--pack-destination",
      directory,
    ]);
  const archives = readdirSync(directory),
    contract = archives.find(
      (f) => f.startsWith("rss-mdm-agent-ai-contract-") && f.endsWith(".tgz"),
    ),
    adapter = archives.find(
      (f) =>
        f.startsWith("rss-mdm-agent-ai-adapter-claude-") && f.endsWith(".tgz"),
    );
  if (!contract || !adapter) throw new Error("Missing tarballs");
  const versions = JSON.parse(
    readFileSync(join(root, "package.json"), "utf8"),
  ).devDependencies;
  writeFileSync(
    join(directory, "package.json"),
    JSON.stringify({
      name: "isolated-claude-consumer",
      private: true,
      type: "module",
      dependencies: {
        "@rss-mdm-agent/ai-contract": `file:./${contract}`,
        "@rss-mdm-agent/ai-adapter-claude": `file:./${adapter}`,
      },
      devDependencies: {
        typescript: versions.typescript,
        "@types/node": versions["@types/node"],
      },
    }),
  );
  const exceptions =
    readFileSync(join(root, "pnpm-workspace.yaml"), "utf8").split(
      "minimumReleaseAgeExclude:",
    )[1] ?? "";
  writeFileSync(
    join(directory, "pnpm-workspace.yaml"),
    `packages: []\noverrides: ${JSON.stringify({ "@rss-mdm-agent/ai-contract": `file:./${contract}` })}\nminimumReleaseAgeExclude:${exceptions}`,
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
    `import {createClaudeAdapter,type ClaudeAdapterOptions,type ResolvedClaudeConfiguration} from '@rss-mdm-agent/ai-adapter-claude';
import type {ProviderAgentPort} from '@rss-mdm-agent/ai-contract';
const options:ClaudeAdapterOptions={resolveConfiguration:async()=>{throw new Error('not configured');}};
const adapter:ProviderAgentPort=createClaudeAdapter(options);
// @ts-expect-error A Claude resolver cannot claim another provider.
const wrongProvider:ResolvedClaudeConfiguration['configuration']['provider']='codex';
// @ts-expect-error SDK permission bypass is not a product option.
createClaudeAdapter({resolveConfiguration:options.resolveConfiguration,permissionMode:'bypassPermissions'});
await adapter.close({timeoutMs:100,signal:new AbortController().signal});`,
  );
  for (const file of ["adapter.test.mjs", "native.test.mjs"]) {
    const source = readFileSync(
      join(root, "tests/ai-adapters/claude", file),
      "utf8",
    )
      .replaceAll(
        "../../../packages/ai-adapters/claude/dist/testing.js",
        "@rss-mdm-agent/ai-adapter-claude/testing",
      )
      .replaceAll(
        "../../../packages/ai-adapters/claude/dist/index.js",
        "@rss-mdm-agent/ai-adapter-claude",
      )
      .replaceAll(
        "../../../packages/ai-contract/dist/testing/index.js",
        "@rss-mdm-agent/ai-contract/testing",
      )
      .replaceAll(
        "../../../packages/ai-contract/dist/session.js",
        "@rss-mdm-agent/ai-contract/session",
      )
      .replaceAll(
        "../../../packages/ai-contract/dist/index.js",
        "@rss-mdm-agent/ai-contract",
      );
    writeFileSync(join(directory, file), source);
  }
  run("pnpm", ["install", "--offline"]);
  run("pnpm", ["exec", "tsc"]);
  run("node", ["out/consumer.js"]);
  run("node", ["--test", "adapter.test.mjs", "native.test.mjs"]);
  const manifest = JSON.parse(
    readFileSync(
      join(
        directory,
        "node_modules/@rss-mdm-agent/ai-adapter-claude/package.json",
      ),
      "utf8",
    ),
  );
  if (
    Object.keys(manifest.dependencies).some((name) =>
      /tauri|sqlite|execution|vue/.test(name),
    )
  )
    throw new Error("Forbidden adapter dependency");
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
    join(root, ".local-ci-runs/claude-consumer.json"),
    JSON.stringify(
      {
        status: passed ? "passed" : "failed",
        behaviorPassed,
        source: { start, end },
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
      "Claude consumer: clean committed source required for delivery evidence",
    );
}
