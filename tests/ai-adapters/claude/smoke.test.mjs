import assert from "node:assert/strict";
import { test } from "node:test";
import { spawnSync } from "node:child_process";
import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

test("malformed smoke credentials never enter parser diagnostics", () => {
  const directory = mkdtempSync(join(tmpdir(), "rss-smoke-privacy-"));
  try {
    const path = join(directory, "credential.json");
    writeFileSync(
      path,
      '{"env":{"ANTHROPIC_AUTH_TOKEN":"PRIVATE_CANARY"} trailing',
    );
    const result = spawnSync(
      process.execPath,
      [
        fileURLToPath(
          new URL("../../../scripts/smoke-claude.mjs", import.meta.url),
        ),
        "--credential-file",
        path,
      ],
      { encoding: "utf8" },
    );
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /Invalid credential configuration/);
    assert.equal(result.stderr.includes("PRIVATE_CANARY"), false);
    assert.equal(result.stderr.includes(path), false);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
