import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import {
  cpSync,
  mkdirSync,
  mkdtempSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import {
  packHost,
  run,
  runtimeTreeSha256,
  verifyRuntimeIntegrity,
} from "./ai-host-artifacts.mjs";

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
  const orphans = [
    join(root, "apps/ai-host/dist/claude-provider.js"),
    join(root, "apps/ai-host/dist/claude-provider.d.ts"),
  ];
  t.after(() => {
    rmSync(destination, { recursive: true, force: true });
    for (const orphan of orphans) rmSync(orphan, { force: true });
  });
  mkdirSync(join(root, "apps/ai-host/dist"), { recursive: true });
  writeFileSync(orphans[0], "throw new Error('orphaned output');\n");
  writeFileSync(orphans[1], "export declare const orphaned: true;\n");

  run("pnpm", ["build:ai-host"], root);
  const artifacts = packHost(root, destination);
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
