import assert from "node:assert/strict";
import { test } from "node:test";
import { spawnSync } from "node:child_process";
import { mkdtempSync, writeFileSync, rmSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

test("live smoke commands and resume sessions use the current public wire without credentials", async () => {
  const { smokeCommand, smokeSession } = await import(
    "../../../scripts/smoke-claude.mjs"
  );
  const { decode, accessLimits } = await import(
    "../../../packages/ai-contract/dist/index.js"
  );
  const { fixtureSession } = await import(
    "../../../packages/ai-contract/dist/testing/index.js"
  );
  const session = fixtureSession();
  const command = smokeCommand("smoke-turn", "smoke text", 1000);
  const restored = smokeSession(
    session.namespace,
    session.binding,
    session.capabilities,
  );
  assert.equal(
    decode(JSON.stringify(command), accessLimits).schemaVersion,
    session.schemaVersion,
  );
  assert.equal(
    decode(JSON.stringify(restored), accessLimits).schemaVersion,
    session.schemaVersion,
  );
});

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

test("smoke diagnostics retain only closed stage and port classifications", async () => {
  const { describeFailure } = await import("../../../scripts/smoke-claude.mjs");
  assert.deepEqual(
    describeFailure("submit", {
      certainty: "unknown",
      code: "unavailable",
      retry: "reconcile_first",
      message: "PRIVATE_CANARY",
    }),
    {
      stage: "submit",
      certainty: "unknown",
      code: "unavailable",
      retry: "reconcile_first",
    },
  );
  assert.deepEqual(
    describeFailure("terminal", { outcome: "failed", code: "PRIVATE_CANARY" }),
    { stage: "terminal", outcome: "failed" },
  );
});

test("smoke cleanup retries with a fresh budget and retains a live runtime directory", async () => {
  const { closeAdapters } = await import("../../../scripts/smoke-claude.mjs");
  for (const eventuallyStops of [true, false]) {
    const directory = mkdtempSync(join(tmpdir(), "rss-smoke-cleanup-"));
    const signals = [];
    const adapter = {
      close: async (b) => {
        signals.push(b.signal);
        return signals.length === 2 && eventuallyStops
          ? { ok: true, value: { processStopped: true } }
          : {
              ok: false,
              error: { code: "unavailable", retry: "same_command" },
            };
      },
    };
    try {
      const result = await closeAdapters([adapter], directory);
      assert.equal(result.processesStopped, eventuallyStops);
      assert.equal(signals.length, 2);
      assert.notEqual(signals[0], signals[1]);
      assert.equal(existsSync(directory), !eventuallyStops);
      assert.equal(result.directoryRemoved, eventuallyStops);
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  }
});
