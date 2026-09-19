import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import {
  closeAdapters,
  describeFailure,
  loadSmokeConfiguration,
} from "../../../scripts/smoke-codex.mjs";

test("missing and malformed smoke configuration never reports a pass or leaks input", () => {
  assert.throws(
    () => loadSmokeConfiguration([], {}),
    /invalid smoke configuration/,
  );
  const directory = mkdtempSync(join(tmpdir(), "rss-codex-smoke-config-"));
  try {
    const path = join(directory, "private-config.json");
    writeFileSync(path, '{"apiKey":"PRIVATE_CANARY" trailing');
    const result = spawnSync(
      process.execPath,
      [
        fileURLToPath(
          new URL("../../../scripts/smoke-codex.mjs", import.meta.url),
        ),
        "--config-file",
        path,
      ],
      { encoding: "utf8", env: { ...process.env, CI_BASE: "origin/develop" } },
    );
    assert.notEqual(result.status, 0);
    assert.equal(result.stdout.includes("PRIVATE_CANARY"), false);
    assert.equal(result.stderr.includes("PRIVATE_CANARY"), false);
    assert.equal(result.stdout.includes(path), false);
    assert.equal(result.stderr.includes(path), false);
    const summary = JSON.parse(result.stdout.trim().split("\n").at(-1));
    assert.equal(summary.status, "not_run");
    assert.equal(summary.mode, "not_configured");
    assert.equal(summary.behaviorPassed, false);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("smoke mode and endpoint are explicit and mutually bounded", () => {
  const fixture = loadSmokeConfiguration([], {
    RSS_CODEX_SMOKE_MODE: "local_fixture",
    RSS_CODEX_SMOKE_API_URL: "http://127.0.0.1:4123/v1",
    RSS_CODEX_SMOKE_API_KEY: "fixture-only",
    RSS_CODEX_SMOKE_MODEL: "fixture-model",
  });
  assert.equal(fixture.mode, "local_fixture");
  assert.equal(fixture.endpoint.mode, "local_fixture");
  assert.equal("apiKey" in fixture.endpoint, false);
  assert.throws(
    () =>
      loadSmokeConfiguration([], {
        RSS_CODEX_SMOKE_MODE: "real_model",
        RSS_CODEX_SMOKE_API_URL: "http://127.0.0.1:4123/v1",
        RSS_CODEX_SMOKE_API_KEY: "fixture-only",
        RSS_CODEX_SMOKE_MODEL: "fixture-model",
      }),
    /invalid smoke configuration/,
  );
});

test("smoke failures retain only closed classifications", () => {
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
});

test("cleanup retries with fresh budgets and retains a live runtime directory", async () => {
  for (const eventuallyStops of [true, false]) {
    const directory = mkdtempSync(join(tmpdir(), "rss-codex-smoke-cleanup-"));
    const signals = [];
    const adapter = {
      close: async (budget) => {
        signals.push(budget.signal);
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
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  }
});
