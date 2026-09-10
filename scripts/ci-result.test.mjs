import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { stepResult } from "./ci-result.mjs";

test("CI receipt distinguishes success, nonzero exit, signal and spawn failure", () => {
  const run = (script) =>
    stepResult(
      "child",
      [process.execPath],
      spawnSync(process.execPath, ["-e", script]),
    );
  assert.equal(run("process.exit(0)").status, 0);
  assert.equal(run("process.exit(7)").status, 7);
  const killed = JSON.parse(
    JSON.stringify(run("process.kill(process.pid, 'SIGKILL')")),
  );
  assert.equal(killed.status, null);
  assert.equal(killed.signal, "SIGKILL");
  assert.equal(killed.error, "terminated by signal SIGKILL");
  const missing = stepResult(
    "missing",
    [],
    spawnSync("rss-ci-nonexistent-command"),
  );
  assert.equal(missing.signal, null);
  assert.match(missing.error, /ENOENT/);
  assert.equal(
    stepResult("unknown", [], { status: null, signal: null }).error,
    "process ended without an exit status",
  );
});
