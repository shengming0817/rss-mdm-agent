import assert from "node:assert/strict";
import { test } from "node:test";
import {
  mkdtempSync,
  mkdirSync,
  writeFileSync,
  readFileSync,
  readdirSync,
  rmSync,
} from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { publishProtocol } from "../../../scripts/generate-codex-protocol.mjs";

test("protocol check is read-only; write removes stale files and publication failures restore the old closure", () => {
  const root = mkdtempSync(join(tmpdir(), "codex-codegen-"));
  try {
    const destination = join(root, "protocol"),
      manifest = join(root, "manifest.json");
    mkdirSync(destination);
    writeFileSync(join(destination, "old.ts"), "old");
    writeFileSync(manifest, "old manifest");
    const next = new Map([["new.ts", "new"]]);
    assert.throws(
      () => publishProtocol(destination, manifest, next, "new manifest", true),
      /file set drift/,
    );
    assert.equal(readFileSync(join(destination, "old.ts"), "utf8"), "old");
    assert.equal(readFileSync(manifest, "utf8"), "old manifest");
    publishProtocol(destination, manifest, next, "new manifest");
    assert.deepEqual(readdirSync(destination), ["new.ts"]);
    publishProtocol(destination, manifest, next, "new manifest", true);
    // The manifest target cannot be replaced: rollback must restore the full tree.
    assert.throws(() =>
      publishProtocol(
        destination,
        join(root, "missing", "manifest"),
        new Map([["partial.ts", "bad"]]),
        "bad",
      ),
    );
    assert.deepEqual(readdirSync(destination), ["new.ts"]);
    assert.equal(readFileSync(manifest, "utf8"), "new manifest");
    assert.throws(() =>
      publishProtocol(
        destination,
        manifest,
        new Map([["../escape.ts", "bad"]]),
        "bad",
      ),
    );
    assert.equal(
      readdirSync(root).some((name) => name.startsWith(".protocol-stage")),
      false,
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
