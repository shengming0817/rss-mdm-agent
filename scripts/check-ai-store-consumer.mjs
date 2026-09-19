import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
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
  dir = mkdtempSync(join(tmpdir(), "rss-ai-sqlite-consumer-"));
let passed = false,
  failure;
const artifacts = [];
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
  run("pnpm", ["build:ai-store"], root);
  for (const name of ["ai-contract", "ai-store-sqlite"])
    run("pnpm", [
      "--dir",
      join(root, "packages", name),
      "pack",
      "--pack-destination",
      dir,
    ]);
  const archives = readdirSync(dir).filter((p) => p.endsWith(".tgz"));
  for (const name of archives)
    artifacts.push({
      name,
      sha256: createHash("sha256")
        .update(readFileSync(join(dir, name)))
        .digest("hex"),
    });
  const dependencies = Object.fromEntries(
    ["ai-contract", "ai-store-sqlite"].map((name) => {
      const archive = archives.find((p) =>
        p.startsWith(`rss-mdm-agent-${name}-`),
      );
      if (!archive) throw new Error("missing package");
      return [`@rss-mdm-agent/${name}`, `file:./${archive}`];
    }),
  );
  const versions = JSON.parse(
    readFileSync(join(root, "package.json"), "utf8"),
  ).devDependencies;
  writeFileSync(
    join(dir, "package.json"),
    JSON.stringify({
      name: "isolated-ai-sqlite-consumer",
      private: true,
      type: "module",
      dependencies,
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
      include: ["*.ts"],
    }),
  );
  writeFileSync(
    join(dir, "probe.ts"),
    `import assert from 'node:assert/strict';
import {openSqliteStore} from '@rss-mdm-agent/ai-store-sqlite';
const result=openSqliteStore({path:process.argv[2],mode:'open',busyTimeoutMs:25});
assert.equal(result.ok,process.argv[3]==='open');
if(result.ok)assert.equal((await result.value.close({timeoutMs:1000,signal:new AbortController().signal})).ok,true);`,
  );
  writeFileSync(
    join(dir, "consumer.ts"),
    `import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {mkdtempSync,rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {openSqliteStore} from '@rss-mdm-agent/ai-store-sqlite';
import type {SessionStore} from '@rss-mdm-agent/ai-contract';
import {runStoreConformance,fixtureSession,acceptance,unwrap,restoredSession} from '@rss-mdm-agent/ai-contract/testing';
const dir=mkdtempSync(join(tmpdir(),'isolated-ai-data-'));let i=0;
const budget=()=>({timeoutMs:1000,signal:new AbortController().signal});
try {
 await runStoreConformance(()=>unwrap(openSqliteStore({path:join(dir,'fixture-'+(i++)+'.sqlite'),mode:'create'})));
 const path=join(dir,'owner.sqlite');let store:SessionStore=unwrap(openSqliteStore({path,mode:'create'}));
 const initial=fixtureSession();initial.capabilities.continuation='across_processes';unwrap(await store.create(initial));
 const receipt=unwrap(await store.accept(acceptance(initial)));
 const probe=fileURLToPath(new URL('./probe.js',import.meta.url));
 assert.equal(spawnSync(process.execPath,[probe,path,'blocked'],{stdio:'inherit'}).status,0);
 unwrap(await store.close(budget()));
 assert.equal(spawnSync(process.execPath,[probe,path,'open'],{stdio:'inherit'}).status,0);
 store=unwrap(openSqliteStore({path,mode:'open'}));assert.deepEqual(unwrap(await store.accept(acceptance(initial))),receipt);
 const before=unwrap(await store.snapshot(initial.namespace,1024));
 const restored=await restoredSession(before.session,'isolated-restored');
 const current=unwrap(await store.rebind({namespace:initial.namespace,expectedRevision:before.session.revision,expectedGeneration:initial.binding.generation,restored,eventId:'restore'}));
 assert.equal(current.binding.generation,'isolated-restored');unwrap(await store.close(budget()));
 console.log('Isolated SQLite tarballs: types, shared conformance, durable receipt, two-process ownership and verified restart passed');
} finally {rmSync(dir,{recursive:true,force:true});}`,
  );
  run("pnpm", ["install", "--offline"]);
  run("pnpm", ["exec", "tsc"]);
  run("node", ["out/consumer.js"]);
  const manifest = JSON.parse(
    readFileSync(
      join(dir, "node_modules/@rss-mdm-agent/ai-store-sqlite/package.json"),
      "utf8",
    ),
  );
  if (
    Object.keys(manifest.dependencies ?? {}).length !== 0 ||
    manifest.peerDependencies["@rss-mdm-agent/ai-contract"] !== "0.1.0"
  )
    throw new Error("unexpected runtime dependency closure");
  passed = true;
} catch (error) {
  failure = String(error);
  process.exitCode = 1;
} finally {
  rmSync(dir, { recursive: true, force: true });
  const end = sourceState(root),
    deliverable = passed && sameCommittedSource(start, end);
  if (!deliverable) process.exitCode = 1;
  mkdirSync(join(root, ".local-ci-runs"), { recursive: true });
  writeFileSync(
    join(root, ".local-ci-runs/ai-store-consumer.json"),
    JSON.stringify(
      {
        status: deliverable ? "passed" : "failed",
        behaviorPassed: passed,
        source: { start, end },
        artifacts,
        lockSha256: createHash("sha256")
          .update(readFileSync(join(root, "pnpm-lock.yaml")))
          .digest("hex"),
        platform: process.platform,
        arch: process.arch,
        node: process.version,
        sqlite: process.versions.sqlite,
        failure,
      },
      null,
      2,
    ),
  );
  if (failure) console.error(failure);
  if (!sameCommittedSource(start, end))
    console.error(
      "SQLite consumer: delivery evidence requires clean committed source",
    );
}
