import { test } from "node:test";
import assert from "node:assert/strict";
import {
  mkdtempSync,
  mkdirSync,
  copyFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { runtimeTreeSha256 } from "./ai-host-artifacts.mjs";
import { freezeCandidate, verifyCandidate } from "./native-candidate.mjs";
test(
  "frozen signed candidate rejects replaced bytes and different platform",
  { skip: process.platform !== "darwin" },
  (t) => {
    const root = mkdtempSync(join(tmpdir(), "agent-candidate-"));
    t.after(() => rmSync(root, { recursive: true, force: true }));
    const binary = join(root, "service");
    copyFileSync("/bin/sh", binary);
    const runtime = join(root, "runtime");
    mkdirSync(join(runtime, "bin"), { recursive: true });
    mkdirSync(join(runtime, "node_modules"));
    for (const name of [
      "NODE-LICENSE",
      "package.json",
      "pnpm-lock.yaml",
      "worker-manifest.json",
    ])
      writeFileSync(join(runtime, name), "{}");
    writeFileSync(join(runtime, "bin/node"), "fixed runtime");
    writeFileSync(
      join(runtime, "manifest.json"),
      JSON.stringify({ runtimeTreeSha256: runtimeTreeSha256(runtime) }),
    );
    const candidate = freezeCandidate(process.cwd(), {
      service: binary,
      backend: binary,
      desktop: binary,
      runtime,
    });
    const file = join(root, "candidate.json");
    writeFileSync(file, JSON.stringify(candidate));
    assert.equal(
      verifyCandidate(process.cwd(), file).binaries.service.sha256,
      candidate.binaries.service.sha256,
    );
    const manifest = JSON.stringify({
      runtimeTreeSha256: runtimeTreeSha256(runtime),
    });
    writeFileSync(join(runtime, "manifest.json"), "substituted manifest");
    assert.throws(() => verifyCandidate(process.cwd(), file));
    writeFileSync(join(runtime, "manifest.json"), manifest);
    writeFileSync(join(runtime, "bin/node"), "substituted runtime");
    assert.throws(() => verifyCandidate(process.cwd(), file));
    writeFileSync(join(runtime, "bin/node"), "fixed runtime");
    writeFileSync(file, JSON.stringify({ ...candidate, platform: "win32" }));
    assert.throws(() => verifyCandidate(process.cwd(), file));
    writeFileSync(file, JSON.stringify(candidate));
    writeFileSync(binary, "substituted image");
    assert.throws(() => verifyCandidate(process.cwd(), file));
  },
);
