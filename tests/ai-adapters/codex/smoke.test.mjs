import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
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

test("real-model smoke accepts configured HTTPS endpoints and rejects model identity mismatch", async () => {
  const { modelReceipt, startSmokeModelGateway } = await import(
    "../../../scripts/codex-smoke-model.mjs"
  );
  const env = {
    RSS_CODEX_SMOKE_MODE: "real_model",
    RSS_CODEX_SMOKE_API_URL: "https://untrusted.example/v1",
    RSS_CODEX_SMOKE_API_KEY: "SECRET_CANARY",
    RSS_CODEX_SMOKE_MODEL: "expected",
  };
  assert.equal(
    loadSmokeConfiguration([], env).apiUrl,
    "https://untrusted.example/v1",
  );
  const good = loadSmokeConfiguration([], {
    ...env,
    RSS_CODEX_SMOKE_API_URL: "https://api.openai.com/v1",
  });
  assert.equal(good.mode, "real_model");
  await assert.rejects(
    startSmokeModelGateway({ ...good, apiUrl: "http://untrusted.example/v1" }),
    /untrusted/,
  );
  const event = {
    type: "response.completed",
    response: { id: "resp_test", status: "completed", model: "expected" },
  };
  assert.equal(modelReceipt(event, "expected").backend, "configured_endpoint");
  for (const response of [
    { ...event.response, model: "wrong" },
    { ...event.response, model: undefined },
    { ...event.response, status: "incomplete" },
    { ...event.response, id: undefined },
  ]) {
    assert.throws(
      () => modelReceipt({ ...event, response }, "expected"),
      /identity mismatch/,
    );
  }
  assert.equal(
    JSON.stringify(modelReceipt(event, "expected")).includes("expected"),
    false,
  );
});

test("continuity prompts reveal a fresh nonce only in the initial turn", async () => {
  const { continuityChallenge } = await import(
    "../../../scripts/codex-smoke-model.mjs"
  );
  const first = continuityChallenge(),
    second = continuityChallenge();
  assert.notEqual(first.expected, second.expected);
  assert.ok(first.prompts[0].includes(first.expected));
  assert.ok(
    first.prompts.slice(1).every((prompt) => !prompt.includes(first.expected)),
  );
  // A responder with only the current prompt cannot recover either follow-up answer.
  assert.deepEqual(first.prompts.slice(1), second.prompts.slice(1));
});

test(
  "native smoke needs history: a stateless responder fails the second turn",
  { timeout: 30000 },
  async (t) => {
    const { nativeFixture, reply } = await import("./helpers.mjs");
    for (const remembers of [false, true]) {
      let initial;
      const fixture = await nativeFixture(t, {
        handleModel: (body, response, index) => {
          const users = (body.input ?? []).filter(
            (item) => item.role === "user",
          );
          const last = JSON.stringify(users.at(-1));
          const current = last?.match(/session marker: ([a-f0-9]{48})/i)?.[1];
          if (index === 1) {
            assert.ok(current);
            initial = current;
          }
          if (index > 1) assert.equal(current, undefined);
          const recalled = JSON.stringify(users).match(
            /session marker: ([a-f0-9]{48})/i,
          )?.[1];
          reply(response, (remembers ? recalled : current) ?? "NO_CONTEXT");
        },
      });
      const settings = await fixture.options.resolveConfiguration({
        namespace: fixture.configuration.namespace,
      });
      const child = spawn(
        process.execPath,
        [
          fileURLToPath(
            new URL("../../../scripts/smoke-codex.mjs", import.meta.url),
          ),
        ],
        {
          env: {
            ...process.env,
            CI_BASE: "origin/develop",
            RSS_CODEX_SMOKE_MODE: "local_fixture",
            RSS_CODEX_SMOKE_API_URL: settings.authentication.apiUrl,
            RSS_CODEX_SMOKE_API_KEY: settings.authentication.apiKey,
            RSS_CODEX_SMOKE_MODEL: settings.model,
          },
          stdio: ["ignore", "pipe", "pipe"],
        },
      );
      let stdout = "",
        stderr = "";
      child.stdout.on("data", (data) => (stdout += data));
      child.stderr.on("data", (data) => (stderr += data));
      await new Promise((resolve, reject) => {
        child.once("error", reject);
        child.once("close", resolve);
      });
      const result = JSON.parse(stdout.trim().split("\n").at(-1));
      assert.equal(result.behaviorPassed, remembers, stderr);
      assert.equal(result.results.length, remembers ? 3 : 1);
      assert.equal(result.processesStopped, true);
      assert.equal(stdout.includes(initial), false);
      if (!remembers) assert.equal(result.failure.stage, "text");
    }
  },
);

test("identity relay validates the actual streamed responses and fails closed on mismatches", async (t) => {
  const https = (await import("node:https")).default;
  const { syncBuiltinESMExports } = await import("node:module");
  const { EventEmitter } = await import("node:events");
  const { Readable } = await import("node:stream");
  const { rootCertificates } = await import("node:tls");
  const { startSmokeModelGateway } = await import(
    "../../../scripts/codex-smoke-model.mjs"
  );
  let returnedModel = "expected",
    requests = 0;
  t.mock.method(https, "request", (url, options, onResponse) => {
    assert.equal(url, "https://api.openai.com/v1/responses");
    assert.equal(options.rejectUnauthorized, true);
    assert.equal(options.ca, rootCertificates);
    const upstream = new EventEmitter();
    upstream.destroy = () => {};
    upstream.end = (body) => {
      assert.equal(JSON.parse(body).model, "expected");
      const event = {
        type: "response.completed",
        response: {
          id: `resp_${++requests}`,
          status: "completed",
          model: returnedModel,
        },
      };
      const response = Readable.from([`data: ${JSON.stringify(event)}\n\n`]);
      response.statusCode = 200;
      response.headers = { "content-type": "text/event-stream" };
      void onResponse(response);
    };
    return upstream;
  });
  syncBuiltinESMExports();
  t.after(() => {
    t.mock.restoreAll();
    syncBuiltinESMExports();
  });
  for (const mismatch of [false, true]) {
    const gateway = await startSmokeModelGateway({
      mode: "real_model",
      apiUrl: "https://api.openai.com/v1",
      apiKey: "TEST_SECRET",
      model: "expected",
    });
    try {
      const denied = await fetch(`${gateway.apiUrl}/responses`, {
        method: "POST",
        body: "{}",
        signal: AbortSignal.timeout(2000),
      });
      assert.equal(denied.status, 403);
      for (let i = 0; i < 3; i++) {
        returnedModel = mismatch && i === 2 ? "wrong" : "expected";
        const result = await fetch(`${gateway.apiUrl}/responses`, {
          method: "POST",
          headers: { authorization: `Bearer ${gateway.apiKey}` },
          body: JSON.stringify({ model: "expected", stream: true }),
          signal: AbortSignal.timeout(2000),
        })
          .then((r) => r.text())
          .catch(() => "rejected");
        if (returnedModel === "wrong") assert.equal(result, "rejected");
      }
      assert.equal(await gateway.verify(), !mismatch);
      assert.equal(
        JSON.stringify(gateway.receipts).includes("TEST_SECRET"),
        false,
      );
      assert.equal(
        JSON.stringify(gateway.receipts).includes("expected"),
        false,
      );
    } finally {
      await gateway.close();
    }
  }
});
