import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import { fileURLToPath } from "node:url";
import { sameCommittedSource, sourceState } from "./source-state.mjs";

const root = fileURLToPath(new URL("../", import.meta.url));
const start = sourceState(root);
const directory = mkdtempSync(join(tmpdir(), "rss-codex-consumer-"));
let stage = "build";
let behaviorPassed = false;
let runtime = { processStopped: false };

function run(command, args, cwd = directory) {
  const result = spawnSync(command, args, {
    cwd,
    stdio: "inherit",
    env: { ...process.env, NODE_PATH: "", PNPM_WORKSPACE_DIR: "" },
  });
  if (result.status !== 0) throw new Error("consumer command failed");
}

const sha256 = (path) =>
  createHash("sha256").update(readFileSync(path)).digest("hex");

try {
  run("pnpm", ["build:ai-contract"], root);
  run("pnpm", ["--filter", "@rss-mdm-agent/ai-adapter-codex", "build"], root);
  stage = "pack";
  for (const pkg of ["packages/ai-contract", "packages/ai-adapters/codex"])
    run("pnpm", [
      "--dir",
      join(root, pkg),
      "pack",
      "--pack-destination",
      directory,
    ]);
  const archives = readdirSync(directory);
  const contract = archives.find(
    (file) =>
      file.startsWith("rss-mdm-agent-ai-contract-") && file.endsWith(".tgz"),
  );
  const adapter = archives.find(
    (file) =>
      file.startsWith("rss-mdm-agent-ai-adapter-codex-") &&
      file.endsWith(".tgz"),
  );
  if (!contract || !adapter) throw new Error("missing packed package");
  const versions = JSON.parse(
    readFileSync(join(root, "package.json"), "utf8"),
  ).devDependencies;
  writeFileSync(
    join(directory, "package.json"),
    JSON.stringify({
      name: "isolated-codex-consumer",
      private: true,
      type: "module",
      dependencies: {
        "@rss-mdm-agent/ai-contract": `file:./${contract}`,
        "@rss-mdm-agent/ai-adapter-codex": `file:./${adapter}`,
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
    `import assert from 'node:assert/strict';
import {chmodSync,mkdirSync,writeFileSync} from 'node:fs';
import {join} from 'node:path';
import {createCodexAdapter,CODEX_VERSION,type CodexAdapterOptions,type ResolvedCodexConfiguration} from '@rss-mdm-agent/ai-adapter-codex';
import type {ProviderAgentPort} from '@rss-mdm-agent/ai-contract';
const project=join(process.cwd(),'project'),native=join(process.cwd(),'native');
mkdirSync(project);mkdirSync(native,{mode:0o700});chmodSync(native,0o700);
const configuration={provider:'codex',config:{id:'consumer',revision:'1'},workingDirectory:project,namespace:{tenantId:'consumer',principalId:'consumer',authorityId:'consumer',sessionId:'consumer-session'},permissions:'tools_disabled'} as const;
const options:CodexAdapterOptions={resolveConfiguration:async()=>({configuration,nativeDirectory:native,authentication: { type: "api_key", apiUrl:'http://127.0.0.1:9/v1', apiKey:'fixture-only' },model:'fixture-model'})};
const adapter=createCodexAdapter(options);const port:ProviderAgentPort=adapter.agent;
// @ts-expect-error Native history is not a public provider operation.
port.readHistory;
// @ts-expect-error Host admission is not a provider operation.
port.fork;
// @ts-expect-error Diagnostics expose no business payload.
const unsafeDiagnostic: import('@rss-mdm-agent/ai-contract').ProviderDiagnostic = {kind:'other', dropped:0, message:{}};
void unsafeDiagnostic;
// @ts-expect-error A Codex resolver cannot claim another provider.
const wrongProvider:ResolvedCodexConfiguration['configuration']['provider']='claude';
// @ts-expect-error Native binary and arbitrary app-server options are sealed.
createCodexAdapter({resolveConfiguration:options.resolveConfiguration,binaryPath:'/tmp/codex'});
const budget=()=>({timeoutMs:15000,signal:AbortSignal.timeout(15000)});
let processStopped=false,nativeSession=false,nativeThread=false;
try{
  const opened=await port.createSession(configuration,budget());assert.equal(opened.ok,true);
  if(!opened.ok)throw new Error('open failed');
  assert.equal(opened.value.binding.providerVersion,CODEX_VERSION);
  nativeSession=Boolean(opened.value.binding.nativeSessionId);nativeThread=Boolean(opened.value.binding.nativeThreadId);
  assert.ok(nativeSession);assert.ok(nativeThread);
}finally{
  const closed=await port.close(budget());processStopped=closed.ok&&closed.value.processStopped===true;
  writeFileSync('runtime-result.json',JSON.stringify({processStopped,version:CODEX_VERSION,nativeSession,nativeThread}));
  assert.equal(processStopped,true);
}`,
  );
  stage = "install";
  run("pnpm", ["install", "--offline"]);
  stage = "compile";
  run("pnpm", ["exec", "tsc"]);
  stage = "runtime";
  run("node", ["out/consumer.js"]);
  runtime = JSON.parse(
    readFileSync(join(directory, "runtime-result.json"), "utf8"),
  );
  if (
    runtime.processStopped !== true ||
    runtime.nativeSession !== true ||
    runtime.nativeThread !== true ||
    runtime.version !== "0.155.0"
  )
    throw new Error("invalid runtime proof");
  const manifestPath = join(
    directory,
    "node_modules/@rss-mdm-agent/ai-adapter-codex/package.json",
  );
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  if (
    manifest.dependencies["@openai/codex"] !== "0.155.0" ||
    Object.keys(manifest.dependencies).some((name) =>
      /tauri|sqlite|execution|vue/.test(name),
    ) ||
    !existsSync(
      join(
        directory,
        "node_modules/@rss-mdm-agent/ai-adapter-codex/protocol-manifest.json",
      ),
    )
  )
    throw new Error("invalid installed package");
  behaviorPassed = true;
} catch {
  process.exitCode = 1;
} finally {
  const end = sourceState(root);
  const deliverable =
    behaviorPassed &&
    runtime.processStopped === true &&
    sameCommittedSource(start, end);
  if (!deliverable) process.exitCode = 1;
  const archiveNames = readdirSync(directory).filter((file) =>
    file.endsWith(".tgz"),
  );
  const packageSha256 = Object.fromEntries(
    archiveNames.map((file) => [
      file.includes("adapter-codex") ? "adapter" : "contract",
      sha256(join(directory, file)),
    ]),
  );
  mkdirSync(join(root, ".local-ci-runs"), { recursive: true });
  writeFileSync(
    join(root, ".local-ci-runs/codex-consumer.json"),
    JSON.stringify(
      {
        evidence: "isolated-packed-codex-consumer",
        status: deliverable ? "passed" : "failed",
        behaviorPassed,
        processStopped: runtime.processStopped === true,
        stage: behaviorPassed ? "complete" : stage,
        source: { start, end },
        lockSha256: sha256(join(root, "pnpm-lock.yaml")),
        packageSha256,
        codex: runtime.version ?? "not_started",
        platform: process.platform,
        arch: process.arch,
        node: process.version,
      },
      null,
      2,
    ),
  );
  if (runtime.processStopped === true || stage !== "runtime")
    rmSync(directory, { recursive: true, force: true });
  else console.error(`Codex consumer runtime retained: ${basename(directory)}`);
  if (!sameCommittedSource(start, end))
    console.error(
      "Codex consumer: clean committed source required for delivery evidence",
    );
}
