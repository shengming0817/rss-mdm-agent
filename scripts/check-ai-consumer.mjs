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
const root = fileURLToPath(new URL("../", import.meta.url));
const start = sourceState(root),
  dir = mkdtempSync(join(tmpdir(), "rss-ai-consumer-"));
let passed = false,
  failure;
function run(command, args, cwd = dir) {
  const result = spawnSync(command, args, {
    cwd,
    stdio: "inherit",
    env: { ...process.env, NODE_PATH: "", PNPM_WORKSPACE_DIR: "" },
  });
  if (result.status !== 0)
    throw new Error(
      `${command}: ${result.status ?? result.error?.code ?? result.signal}`,
    );
}
try {
  run("pnpm", ["build:ai-contract"], root);
  run("pnpm", [
    "--dir",
    join(root, "packages/ai-contract"),
    "pack",
    "--pack-destination",
    dir,
  ]);
  const archive = readdirSync(dir).find((p) => p.endsWith(".tgz"));
  if (!archive) throw new Error("packed archive missing");
  const versions = JSON.parse(
    readFileSync(join(root, "package.json"), "utf8"),
  ).devDependencies;
  writeFileSync(
    join(dir, "package.json"),
    JSON.stringify({
      name: "isolated-ai-consumer",
      private: true,
      type: "module",
      dependencies: { "@rss-mdm-agent/ai-contract": `file:./${archive}` },
      devDependencies: {
        typescript: versions.typescript,
        "@types/node": versions["@types/node"],
      },
    }),
  );
  writeFileSync(join(dir, "pnpm-workspace.yaml"), "packages: []\n");
  writeFileSync(
    join(dir, "tsconfig.json"),
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
    join(dir, "consumer.ts"),
    `import assert from 'node:assert/strict';
import {decode,fingerprint,type HostPort,type ProviderAgentPort,type SessionStore} from '@rss-mdm-agent/ai-contract';
import {FakeHost,MemorySessionStore,ScriptedProvider,fixtures,fixtureLimits,runStoreConformance,runProviderConformance,runHostConformance} from '@rss-mdm-agent/ai-contract/testing';
const host:HostPort=new FakeHost();const provider:ProviderAgentPort=new ScriptedProvider();const store:SessionStore=new MemorySessionStore();
const value=decode(JSON.stringify(fixtures.valid[0]),fixtureLimits);assert.equal(value.kind,'command');if(value.kind==='command')assert.equal(fingerprint(value,fixtureLimits),fixtures.commandHash);
await runStoreConformance(()=>new MemorySessionStore());
await runProviderConformance(scenario=>{const port=new ScriptedProvider();port.submission=scenario;return port;},{config:{id:'config-1',revision:'1'},accountRef:'account-1',workingDirectory:'.',permissions:'tools_disabled'},{timeoutMs:1000,signal:AbortSignal.timeout(1000)});
await runHostConformance(()=>new FakeHost());
assert.equal(host.negotiate({contractVersion:2,acp:1,durableReceipts:false,cursorAttach:false}).ok,true);
assert.ok(store);console.log('Isolated AI tarball consumer: types, wire, Host and conformance passed');`,
  );
  run("pnpm", ["install", "--offline"]);
  run("pnpm", ["exec", "tsc"]);
  run("node", ["out/consumer.js"]);
  const manifest = JSON.parse(
    readFileSync(
      join(dir, "node_modules/@rss-mdm-agent/ai-contract/package.json"),
      "utf8",
    ),
  );
  if (
    Object.keys(manifest.dependencies ?? {}).some((n) =>
      /tauri|sqlite|codex|claude|vue/.test(n),
    )
  )
    throw new Error("unexpected runtime dependency");
  passed = true;
} catch (error) {
  failure = String(error);
  process.exitCode = 1;
} finally {
  rmSync(dir, { recursive: true, force: true });
  const end = sourceState(root);
  const deliverable = passed && sameCommittedSource(start, end);
  if (!deliverable) process.exitCode = 1;
  mkdirSync(join(root, ".local-ci-runs"), { recursive: true });
  writeFileSync(
    join(root, ".local-ci-runs/ai-consumer.json"),
    JSON.stringify(
      {
        status: deliverable ? "passed" : "failed",
        behaviorPassed: passed,
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
      "AI consumer: source must be clean and committed for delivery evidence",
    );
}
