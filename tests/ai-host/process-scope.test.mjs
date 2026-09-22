import test from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { validScope } from "../../packages/ai-host/dist/launch-fence.js";
import { scopeAbsentWithin } from "../../packages/ai-host/dist/process.js";
test("Windows persistent scope requires its OS login session", () => {
  const name = "Local\\rss-mdm-worker-00000000-0000-0000-0000-000000000000";
  assert.equal(validScope({ kind: "jobObject", name }), false);
  assert.equal(validScope({ kind: "jobObject", name, session: 1 }), true);
  assert.equal(validScope({ kind: "jobObject", name, session: -1 }), false);
});
test("a hanging scope probe cannot monopolize the Host event loop or close budget", async () => {
  execFileSync(
    "cargo",
    ["build", "--locked", "-p", "native-process", "--example", "host-fixture"],
    { stdio: "ignore" },
  );
  const launcher = fileURLToPath(
    new URL(
      "../../target/debug/examples/host-fixture" +
        (process.platform === "win32" ? ".exe" : ""),
      import.meta.url,
    ),
  );
  const before = performance.now();
  let tick = false;
  setTimeout(() => {
    tick = true;
  }, 5);
  assert.equal(
    await scopeAbsentWithin(
      { launcher, manifestDigest: "a".repeat(64) },
      { kind: "processGroup", root: 42 },
      { timeoutMs: 30, signal: new AbortController().signal },
    ),
    false,
  );
  assert.equal(tick, true);
  assert.ok(performance.now() - before < 1000);
});
