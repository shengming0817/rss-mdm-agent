import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { sourceState, sameCommittedSource, git } from "./source-state.mjs";
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

test("provenance rejects a base ref advance even when the merge-base is unchanged", () => {
  const cwd = mkdtempSync(join(tmpdir(), "ci-base-"));
  const run = (...args) =>
    execFileSync(git, args, { cwd, encoding: "utf8" }).trim();
  const commit = (message) =>
    run(
      "-c",
      "user.name=Test",
      "-c",
      "user.email=test@example.invalid",
      "commit",
      "--allow-empty",
      "-qm",
      message,
    );
  try {
    run("init", "-q");
    commit("root");
    const root = run("rev-parse", "HEAD");
    run("branch", "baseline");
    commit("feature");
    const before = sourceState(cwd, "baseline");
    const nextBase = execFileSync(
      git,
      [
        "-c",
        "user.name=Test",
        "-c",
        "user.email=test@example.invalid",
        "commit-tree",
        run("rev-parse", root + "^{tree}"),
        "-p",
        root,
      ],
      { cwd, input: "advance baseline\n", encoding: "utf8" },
    ).trim();
    run("update-ref", "refs/heads/baseline", nextBase);
    const after = sourceState(cwd, "baseline");
    assert.equal(before.base, after.base);
    assert.notEqual(before.baseOid, after.baseOid);
    assert.equal(sameCommittedSource(before, after), false);
  } finally {
    rmSync(cwd, { recursive: true, force: true });
  }
});
