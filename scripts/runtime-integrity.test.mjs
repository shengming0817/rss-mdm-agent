import assert from "node:assert/strict";
import { execFileSync, spawnSync, spawn } from "node:child_process";
import {
  cpSync,
  chmodSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { load } from "js-yaml";
import {
  materializeDeploymentLock,
  packHost,
  run,
  runtimeTreeSha256,
  verifyRuntimeIntegrity,
} from "./ai-host-artifacts.mjs";

import {
  withPreparation,
  preparedContext,
  contextKey,
} from "./preparation.mjs";

const roots = [
  "bin",
  "node_modules",
  "NODE-LICENSE",
  "package.json",
  "pnpm-lock.yaml",
  "worker-manifest.json",
];

function runtimeFixture(parent, name) {
  const directory = join(parent, name);
  mkdirSync(join(directory, "bin"), { recursive: true });
  mkdirSync(
    join(directory, "node_modules/.pnpm/example@1.0.0/node_modules/example"),
    { recursive: true },
  );
  writeFileSync(join(directory, "bin/node"), "node-binary\n", { mode: 0o755 });
  writeFileSync(
    join(directory, "bin/rss-ai-worker-launcher"),
    "launcher-fixture\n",
    {
      mode: 0o755,
    },
  );
  writeFileSync(
    join(
      directory,
      "node_modules/.pnpm/example@1.0.0/node_modules/example/index.js",
    ),
    "export const value = 1;\n",
  );
  symlinkSync(
    ".pnpm/example@1.0.0/node_modules/example",
    join(directory, "node_modules/example"),
  );
  writeFileSync(join(directory, "NODE-LICENSE"), "license\n");
  writeFileSync(join(directory, "package.json"), '{"private":true}\n');
  writeFileSync(join(directory, "pnpm-lock.yaml"), "lockfileVersion: '9.0'\n");
  writeFileSync(join(directory, "worker-manifest.json"), "{}\n");
  return directory;
}

test("runtime digest survives verbatim staging and rejects changed JS and binaries", (t) => {
  const parent = mkdtempSync(join(tmpdir(), "rss-runtime-integrity-"));
  t.after(() => rmSync(parent, { recursive: true, force: true }));
  const source = runtimeFixture(parent, "source");
  const digest = runtimeTreeSha256(source);
  assert.match(digest, /^[a-f0-9]{64}$/);
  assert.equal(verifyRuntimeIntegrity(source, digest), digest);
  writeFileSync(
    join(source, "manifest.json"),
    '{"runtimeTreeSha256":"pending"}\n',
  );
  assert.equal(
    runtimeTreeSha256(source),
    digest,
    "manifest is not self-hashed",
  );

  const staged = join(parent, "staged");
  mkdirSync(staged);
  for (const name of roots)
    cpSync(join(source, name), join(staged, name), {
      recursive: true,
      verbatimSymlinks: true,
    });
  assert.equal(verifyRuntimeIntegrity(staged, digest), digest);

  const javascript = join(
    staged,
    "node_modules/.pnpm/example@1.0.0/node_modules/example/index.js",
  );
  writeFileSync(javascript, "export const value = 2;\n");
  assert.throws(
    () => verifyRuntimeIntegrity(staged, digest),
    /runtime tree integrity mismatch/,
  );

  writeFileSync(javascript, "export const value = 1;\n");
  assert.equal(verifyRuntimeIntegrity(staged, digest), digest);
  writeFileSync(join(staged, "bin/node"), "tampered-binary\n", {
    mode: 0o755,
  });
  assert.throws(
    () => verifyRuntimeIntegrity(staged, digest),
    /runtime tree integrity mismatch/,
  );
});

test("AI host build removes orphaned compiler output before packing", (t) => {
  const root = fileURLToPath(new URL("../", import.meta.url));
  const destination = mkdtempSync(join(tmpdir(), "rss-ai-host-pack-"));
  const isolatedRoot = join(destination, "isolated");
  const isolated = join(isolatedRoot, "apps/ai-host");
  mkdirSync(join(isolatedRoot, "apps"), { recursive: true });
  for (const name of [
    "packages",
    "scripts",
    "node_modules",
    "package.json",
    "pnpm-lock.yaml",
    "pnpm-workspace.yaml",
    "patches",
  ])
    symlinkSync(join(root, name), join(isolatedRoot, name));
  cpSync(join(root, "apps/ai-host"), isolated, {
    recursive: true,
    filter: (source) => !["node_modules", "dist"].includes(basename(source)),
  });
  symlinkSync(
    join(root, "apps/ai-host/node_modules"),
    join(isolated, "node_modules"),
    "dir",
  );
  const orphans = [
    join(isolated, "dist/claude-provider.js"),
    join(isolated, "dist/claude-provider.d.ts"),
  ];
  t.after(() => {
    rmSync(destination, { recursive: true, force: true });
    for (const orphan of orphans) rmSync(orphan, { force: true });
  });
  mkdirSync(join(isolated, "dist"), { recursive: true });
  writeFileSync(orphans[0], "throw new Error('orphaned output');\n");
  writeFileSync(orphans[1], "export declare const orphaned: true;\n");

  // ref: pnpm dist/pnpm.mjs@v11.4.0 (runDepsStatusCheck)
  // This isolated compiler fixture borrows frozen source dependencies. pnpm's
  // default auto-install must not purge that linked installation as a new workspace.
  run(
    "pnpm",
    [
      "--dir",
      isolated,
      "--config.verify-deps-before-run=false",
      "run",
      "build",
    ],
    root,
  );
  assert.ok(
    orphans.every((path) => !existsSync(path)),
    "real Host compilation cleans isolated orphan output",
  );
  const artifacts = packHost(isolatedRoot, destination);
  const deployed = materializeDeploymentLock(isolatedRoot, destination);
  const deploymentPolicy = load(
    readFileSync(join(destination, "pnpm-workspace.yaml"), "utf8"),
  );
  const sourcePolicy = load(
    readFileSync(join(root, "pnpm-workspace.yaml"), "utf8"),
  );
  assert.deepEqual(
    deploymentPolicy.patchedDependencies,
    sourcePolicy.patchedDependencies,
  );
  for (const patch of Object.values(sourcePolicy.patchedDependencies ?? {})) {
    const path = typeof patch === "string" ? patch : patch.path;
    assert.equal(
      readFileSync(join(destination, path), "utf8"),
      readFileSync(join(root, path), "utf8"),
    );
  }
  const source = load(readFileSync(join(root, "pnpm-lock.yaml"), "utf8"));
  for (const [key, snapshot] of Object.entries(source.snapshots))
    assert.deepEqual(
      deployed.snapshots[key],
      snapshot,
      `deployment preserves source-locked peer and optional edges for ${key}`,
    );
  for (const [key, pkg] of Object.entries(source.packages))
    assert.deepEqual(deployed.packages[key], pkg, `locked resolution ${key}`);
  const sourceFixture = join(destination, "source");
  mkdirSync(sourceFixture);
  for (const name of ["apps", "packages"])
    symlinkSync(join(root, name), join(sourceFixture, name), "dir");
  delete source.importers["packages/ai-adapters/claude"].dependencies.zod;
  writeFileSync(join(sourceFixture, "pnpm-lock.yaml"), JSON.stringify(source));
  assert.throws(
    () => materializeDeploymentLock(sourceFixture, destination),
    /unlocked direct edge @rss-mdm-agent\/ai-adapter-claude: zod/,
    "missing source lock entries fail before installing any packages",
  );
  const archive = artifacts.find((name) =>
    name.startsWith("rss-mdm-agent-ai-host-app-"),
  );
  assert.ok(archive, "AI host application archive is present");
  const entries = execFileSync(
    "/usr/bin/tar",
    ["-tzf", join(destination, basename(archive))],
    { encoding: "utf8" },
  );
  assert.doesNotMatch(entries, /package\/dist\/claude-provider\.(?:js|d\.ts)/);
});

test("worker staging can replace the same destination twice without stale files or links", async (t) => {
  await withPreparation(["native", "node:@rss-mdm-agent/ai-host"], async () => {
    const root = fileURLToPath(new URL("../", import.meta.url));
    const destination = mkdtempSync(
      join(preparedContext().directory, "worker-repeat-"),
    );
    t.after(() => rmSync(destination, { recursive: true, force: true }));
    run(
      process.execPath,
      ["scripts/build-worker-runtime.mjs", destination],
      root,
    );
    const orphan = join(destination, "worker/orphan.js");
    writeFileSync(orphan, "obsolete output");
    run(
      process.execPath,
      ["scripts/build-worker-runtime.mjs", destination],
      root,
    );
    assert.equal(existsSync(orphan), false);
    assert.ok(
      existsSync(join(destination, "node_modules/@rss-mdm-agent/ai-contract")),
    );
    assert.ok(existsSync(join(destination, "worker/bootstrap.js")));
  });
});

for (const mode of ["failure", "cancellation", "unapproved runtime"])
  test(
    `previous passed runtime is rejected after ${mode}`,
    { timeout: 15000 },
    async (t) => {
      const cancelled = mode === "cancellation";
      const source = fileURLToPath(new URL("../", import.meta.url));
      const root = mkdtempSync(
        join(tmpdir(), "rss-bundle-preparation-failure-"),
      );
      t.after(() => rmSync(root, { recursive: true, force: true }));
      mkdirSync(join(root, "scripts"));
      for (const name of [
        "bundle-ai-host",
        "ai-host-artifacts",
        "desktop-dev-runtime",
        "preparation",
        "ci-plan",
        "ci-result",
        "cargo-target",
        "stage-desktop-runtime",
      ])
        cpSync(
          join(source, `scripts/${name}.mjs`),
          join(root, `scripts/${name}.mjs`),
        );
      cpSync(join(source, "package.json"), join(root, "package.json"));
      symlinkSync(join(source, "node_modules"), join(root, "node_modules"));
      mkdirSync(join(root, "packages/ai-contract/schema"), { recursive: true });
      cpSync(
        join(source, "packages/ai-contract/schema/runtime.schema.json"),
        join(root, "packages/ai-contract/schema/runtime.schema.json"),
      );
      const directory = runtimeFixture(root, ".local-ci-runs/ai-host-runtime");
      writeFileSync(
        join(directory, "manifest.json"),
        JSON.stringify({
          status: "passed",
          kind: "release",
          runtimeTreeSha256: runtimeTreeSha256(directory),
          verification: { platform: process.platform, arch: process.arch },
        }),
      );
      const env = { ...process.env };
      delete env[contextKey];
      delete env._AGENT_BUILD_LEASE;
      const before = spawnSync(
        process.execPath,
        ["scripts/stage-desktop-runtime.mjs"],
        { cwd: root, env, encoding: "utf8" },
      );
      assert.equal(before.status, 0, before.stderr);
      const bin = join(root, "bin"),
        marker = join(root, "preparation-started");
      mkdirSync(bin);
      const command = join(bin, "pnpm");
      writeFileSync(
        command,
        `#!${process.execPath}
import {writeFileSync} from 'node:fs';
if (process.argv.includes('--version')) console.log('11.4.0');
else if (${cancelled}) { writeFileSync(${JSON.stringify(marker)}, 'ready'); setInterval(()=>{}, 1000); }
else { console.error('fixture preparation failure'); process.exit(31); }
`,
      );
      chmodSync(command, 0o755);
      env.PATH = `${bin}:${env.PATH}`;
      if (mode === "unapproved runtime") {
        const manifest = JSON.parse(readFileSync(join(root, "package.json")));
        manifest.engines.node = "0.0.0";
        writeFileSync(join(root, "package.json"), JSON.stringify(manifest));
      }
      let code,
        stderr = "";
      if (cancelled) {
        const child = spawn(process.execPath, ["scripts/bundle-ai-host.mjs"], {
          cwd: root,
          env,
          stdio: ["ignore", "pipe", "pipe"],
        });
        t.after(() => child.kill("SIGKILL"));
        child.stderr.on("data", (bytes) => (stderr += bytes));
        const closed = new Promise((resolve, reject) => {
          child.once("error", reject);
          child.once("close", resolve);
        });
        const start = performance.now();
        while (!existsSync(marker)) {
          assert.ok(performance.now() - start < 5000, stderr);
          await new Promise((resolve) => setTimeout(resolve, 10));
        }
        child.kill("SIGTERM");
        code = await closed;
      } else {
        const result = spawnSync(
          process.execPath,
          ["scripts/bundle-ai-host.mjs"],
          { cwd: root, env, encoding: "utf8" },
        );
        code = result.status;
        stderr = result.stderr;
        assert.match(
          stderr,
          mode === "unapproved runtime"
            ? /approved|artifact|runtime/i
            : /fixture preparation failure/,
        );
      }
      assert.notEqual(code, 0, stderr);
      assert.equal(
        JSON.parse(readFileSync(join(directory, "manifest.json"))).status,
        "failed",
      );
      const after = spawnSync(
        process.execPath,
        ["scripts/stage-desktop-runtime.mjs"],
        { cwd: root, env, encoding: "utf8" },
      );
      assert.notEqual(after.status, 0);
      assert.match(after.stderr, /successfully built runtime/);
    },
  );
