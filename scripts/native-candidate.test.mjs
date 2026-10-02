import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, copyFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { freezeCandidate, verifyCandidate } from "./native-candidate.mjs";
test(
  "frozen signed candidate rejects replaced bytes and different platform",
  { skip: process.platform !== "darwin" },
  (t) => {
    const root = mkdtempSync(join(tmpdir(), "agent-candidate-"));
    t.after(() => rmSync(root, { recursive: true, force: true }));
    const binary = join(root, "service");
    copyFileSync("/bin/sh", binary);
    const candidate = freezeCandidate(process.cwd(), {
      service: binary,
      backend: binary,
    });
    const file = join(root, "candidate.json");
    writeFileSync(file, JSON.stringify(candidate));
    assert.equal(
      verifyCandidate(process.cwd(), file).binaries.service.sha256,
      candidate.binaries.service.sha256,
    );
    writeFileSync(file, JSON.stringify({ ...candidate, platform: "win32" }));
    assert.throws(() => verifyCandidate(process.cwd(), file));
    writeFileSync(file, JSON.stringify(candidate));
    writeFileSync(binary, "substituted image");
    assert.throws(() => verifyCandidate(process.cwd(), file));
  },
);
