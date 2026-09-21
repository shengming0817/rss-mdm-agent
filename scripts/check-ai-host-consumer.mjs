import {
  mkdtempSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
  rmSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";
import { packHost, installArtifacts, run } from "./ai-host-artifacts.mjs";
import { sourceState, sameCommittedSource } from "./source-state.mjs";
const root = fileURLToPath(new URL("../", import.meta.url)),
  start = sourceState(root),
  directory = mkdtempSync(join(tmpdir(), "rss host consumer-"));
let behaviorPassed = false,
  failure,
  deploymentLockSha256,
  artifacts = [];
try {
  run("pnpm", ["build:ai-host"], root);
  artifacts = packHost(root, directory);
  const provider = readFileSync(
    join(root, "tests/ai-host/provider.mjs"),
    "utf8",
  )
    .replace(
      "../../packages/ai-contract/dist/testing/index.js",
      "@rss-mdm-agent/ai-contract/testing",
    )
    .replace(
      "../../packages/ai-contract/dist/session.js",
      "@rss-mdm-agent/ai-contract/session",
    );
  writeFileSync(
    join(directory, "provider.mjs"),
    provider.replaceAll(
      "../../packages/ai-contract/dist/index.js",
      "@rss-mdm-agent/ai-contract",
    ),
  );
  writeFileSync(
    join(directory, "tsconfig.json"),
    JSON.stringify({
      compilerOptions: {
        target: "ES2022",
        module: "NodeNext",
        moduleResolution: "NodeNext",
        strict: true,
        outDir: "out",
        types: ["node"],
      },
      include: ["consumer.ts"],
    }),
  );
  writeFileSync(
    join(directory, "consumer.ts"),
    `import assert from 'node:assert/strict';
import {mkdtemp,rm} from 'node:fs/promises';import {tmpdir} from 'node:os';import {join} from 'node:path';
import {createHost,type HostOptions} from '@rss-mdm-agent/ai-host';import type {WorkerFactory} from '@rss-mdm-agent/ai-host/worker';
import {openSqliteStore} from '@rss-mdm-agent/ai-store-sqlite';import type {Result,HostPort,Command} from '@rss-mdm-agent/ai-contract';
const unwrap=<T>(r:Result<T>):T=>{if(!r.ok)throw new Error(r.error.code);return r.value;};
const dir=await mkdtemp(join(tmpdir(),'isolated-host-db-')),caller={tenantId:'t',principalId:'p',authorityId:'a'},budget=()=>({timeoutMs:5000,signal:new AbortController().signal});
const store=unwrap(openSqliteStore({path:join(dir,'host.sqlite'),mode:'create'}));
const options:HostOptions={delivery:null,store,launchFences:store,resolve:async(caller,options,namespace)=>({configuration:{namespace,provider:options.provider,config:options.config,workingDirectory:dir,permissions:'tools_disabled'},artifact:new URL('../provider.mjs',import.meta.url).href})};
const host:HostPort=unwrap(await createHost(options));
try{unwrap(await store.saveConnection(caller,{schemaVersion:5,kind:'connection',connectionId:'c',name:'Consumer',provider:'codex',configRevision:1,profile:'conversation',status:'ready',source:{type:'custom_api',apiUrl:'https://example.invalid',model:'fixture'}},null));const session=unwrap(await host.createSession(caller,{connectionId:'c'},budget()));
const command:Command={schemaVersion:5,kind:'command',commandId:'prompt',sessionId:session.namespace.sessionId,expiresAtMs:Date.now()+10000,input:{type:'prompt',policy:'queue_next',text:'quick'}};
unwrap(await host.submit(caller,command,budget()));const deadline=Date.now()+5000;let terminal=false;
while(Date.now()<deadline){if(unwrap(await store.command(session.namespace,'prompt')).state==='terminal'){terminal=true;break;}await new Promise(resolve=>setTimeout(resolve,10));}
assert.equal(terminal,true);assert.equal(unwrap(await store.launches()).length,1);
}finally{unwrap(await host.close(budget()));await rm(dir,{recursive:true,force:true});}
console.log('Isolated Host tarballs: typed public API, real SQLite, activated worker, durable terminal and real shutdown passed');`,
  );
  deploymentLockSha256 = installArtifacts(root, directory);
  run("pnpm", ["exec", "tsc"], directory);
  run(process.execPath, ["out/consumer.js"], directory);
  behaviorPassed = true;
} catch (error) {
  failure = String(error);
  process.exitCode = 1;
} finally {
  rmSync(directory, { recursive: true, force: true });
  const end = sourceState(root),
    deliverable = behaviorPassed && sameCommittedSource(start, end);
  if (!deliverable) process.exitCode = 1;
  mkdirSync(join(root, ".local-ci-runs"), { recursive: true });
  writeFileSync(
    join(root, ".local-ci-runs/ai-host-consumer.json"),
    JSON.stringify(
      {
        status: deliverable ? "passed" : "failed",
        behaviorPassed,
        source: { start, end },
        artifacts,
        deploymentLockSha256,
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
      "Host consumer: delivery evidence requires clean committed source",
    );
}
