import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { sourceState, git } from "./source-state.mjs";
test("CI provenance detects untracked, staged and tracked edits and resolves the committed range", () => {
  const cwd = mkdtempSync(join(tmpdir(), "ci-source-"));
  const run = (...args) => execFileSync(git, args, { cwd, stdio: "pipe" });
  try {
    run("init");
    writeFileSync(join(cwd, "file"), "base\n");
    run("add", ".");
    run(
      "-c",
      "user.name=Test",
      "-c",
      "user.email=test@example.com",
      "commit",
      "-m",
      "base",
    );
    const base = sourceState(cwd, "HEAD").head;
    writeFileSync(join(cwd, "new"), "untracked\n");
    assert.equal(sourceState(cwd, base).clean, false);
    run("add", ".");
    assert.equal(sourceState(cwd, base).clean, false);
    run(
      "-c",
      "user.name=Test",
      "-c",
      "user.email=test@example.com",
      "commit",
      "-m",
      "next",
    );
    const state = sourceState(cwd, base);
    assert.equal(state.clean, true);
    assert.equal(state.base, base);
    assert.notEqual(state.head, base);
    writeFileSync(join(cwd, "file"), "dirty\n");
    assert.equal(sourceState(cwd, base).clean, false);
  } finally {
    rmSync(cwd, { recursive: true, force: true });
  }
});
