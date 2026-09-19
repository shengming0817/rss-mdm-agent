import assert from "node:assert/strict";
import test from "node:test";
import { mkdtemp, mkdir, rm, realpath, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createCodexAdapter } from "../../../packages/ai-adapters/codex/dist/index.js";
import { VerifiedProviderSession } from "../../../packages/ai-contract/dist/session.js";
import { fixtureCaller } from "../../../packages/ai-contract/dist/testing/index.js";

const budget = () => ({
  timeoutMs: 2_000,
  signal: new AbortController().signal,
});

async function admission(t, resolved, resolveConfiguration) {
  const adapter = createCodexAdapter({
    resolveConfiguration:
      resolveConfiguration ?? (async () => structuredClone(resolved)),
  });
  t.after(() => rm(resolved.root, { recursive: true, force: true }));
  return VerifiedProviderSession.open(
    adapter.agent,
    resolved.configuration,
    budget(),
  );
}

async function configuration() {
  const root = await realpath(
    await mkdtemp(join(tmpdir(), "rss-codex-admission-")),
  );
  const workingDirectory = join(root, "workspace");
  await mkdir(workingDirectory);
  return {
    root,
    configuration: {
      namespace: { ...fixtureCaller, sessionId: "session-1" },
      provider: "codex",
      config: { id: "cfg", revision: "1" },
      accountRef: "account",
      workingDirectory,
      permissions: "tools_disabled",
    },
    nativeDirectory: join(root, "native"),
    apiUrl: "http://127.0.0.1:9/v1",
    apiKey: "fixture-only",
    model: "fixture",
  };
}

test("permanent launch configuration errors preserve invalid_input", async (t) => {
  for (const [name, mutate] of [
    ["malformed URL", (resolved) => (resolved.apiUrl = "://invalid")],
    [
      "non-loopback HTTP",
      (resolved) => (resolved.apiUrl = "http://example.com/v1"),
    ],
    [
      "relative native directory",
      (resolved) => (resolved.nativeDirectory = "relative-native"),
    ],
  ])
    await t.test(name, async (t) => {
      const resolved = await configuration();
      mutate(resolved);
      const result = await admission(t, resolved);
      assert.deepEqual(result, {
        ok: false,
        error: { code: "invalid_input", retry: "never" },
      });
    });
});

test("foreign native configuration preserves permission_denied", async (t) => {
  const resolved = await configuration();
  await mkdir(resolved.nativeDirectory, { mode: 0o700 });
  await writeFile(
    join(resolved.nativeDirectory, "config.toml"),
    "foreign=true\n",
    {
      mode: 0o600,
    },
  );
  const result = await admission(t, resolved);
  assert.deepEqual(result, {
    ok: false,
    error: { code: "permission_denied", retry: "never" },
  });
});

test("transient configuration resolution failures remain retryable", async (t) => {
  const resolved = await configuration();
  const result = await admission(t, resolved, async () => {
    throw new Error("temporary resolver failure");
  });
  assert.deepEqual(result, {
    ok: false,
    error: { code: "unavailable", retry: "same_command" },
  });
});
