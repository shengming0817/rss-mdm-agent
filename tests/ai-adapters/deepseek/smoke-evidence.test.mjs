import assert from "node:assert/strict";
import test from "node:test";
import { createHash } from "node:crypto";
import { mkdtemp, rm } from "node:fs/promises";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import {
  closeAdapters,
  deliverable,
  smokeConfiguration,
} from "../../../scripts/smoke-deepseek.mjs";
test("smoke endpoint identity distinguishes routes on the same origin", () => {
  const configuration = (path) =>
    smokeConfiguration({
      DEEPSEEK_BASE_URL: `https://private.example/${path}`,
    });
  assert.notDeepEqual(
    configuration("v1").endpoint,
    configuration("v2").endpoint,
  );
  assert.deepEqual(
    configuration("v1").endpoint,
    configuration("discard/../v1/").endpoint,
  );
});
test("smoke evidence describes the endpoint actually selected, without leaking it", () => {
  const official = smokeConfiguration({});
  assert.equal(official.apiUrl, "https://api.deepseek.com");
  assert.equal(official.endpoint.kind, "official");
  const env = {
    DEEPSEEK_BASE_URL: "https://private.example/v1",
    DEEPSEEK_MODEL: "custom",
  };
  const custom = smokeConfiguration(env);
  env.DEEPSEEK_BASE_URL = "https://api.deepseek.com";
  assert.equal(custom.apiUrl, "https://private.example/v1");
  assert.equal(custom.model, "custom");
  assert.equal(custom.endpoint.kind, "configured");
  assert.equal(
    custom.endpoint.endpointSha256,
    createHash("sha256").update(custom.apiUrl).digest("hex"),
  );
  assert.equal("originSha256" in custom.endpoint, false);
  assert.doesNotMatch(JSON.stringify(custom.endpoint), /private\.example/);
  assert.notEqual(
    custom.endpoint.endpointSha256,
    official.endpoint.endpointSha256,
  );
  assert.equal(
    smokeConfiguration({ DEEPSEEK_BASE_URL: "http://127.0.0.1:1234" }).endpoint
      .kind,
    "local_fixture",
  );
  assert.equal(
    smokeConfiguration({
      DEEPSEEK_BASE_URL: "https://api.deepseek.com.example",
    }).endpoint.kind,
    "configured",
  );
  for (const url of [
    "http://private.example",
    "https://secret@api.deepseek.com",
    "https://api.deepseek.com?key=secret",
    "not a URL",
  ]) {
    assert.throws(
      () => smokeConfiguration({ DEEPSEEK_BASE_URL: url }),
      /^Error: Invalid smoke configuration$/,
    );
  }
});
test("smoke success requires behavior and complete cleanup", () => {
  const cleanup = { processesStopped: true, directoryRemoved: true };
  assert.equal(deliverable(true, cleanup), true);
  assert.equal(deliverable(false, cleanup), false);
  for (const field of ["processesStopped", "directoryRemoved"])
    assert.equal(deliverable(true, { ...cleanup, [field]: false }), false);
});
test("smoke cleanup retries failures and still closes later ports before retaining logs", async () => {
  const dir = await mkdtemp(join(tmpdir(), "dsh-smoke-cleanup-"));
  let calls = 0;
  try {
    const cleanup = await closeAdapters(
      [
        {
          close: async () => {
            throw Error("provider secret");
          },
        },
        {
          close: async () => {
            calls++;
            return { ok: true, value: { processStopped: true } };
          },
        },
      ],
      dir,
    );
    assert.equal(calls, 1);
    assert.deepEqual(cleanup, {
      attempts: [2, 1],
      processesStopped: false,
      directoryRemoved: false,
    });
    assert.equal(existsSync(dir), true);
    assert.equal((await closeAdapters([], dir)).directoryRemoved, true);
    assert.equal(existsSync(dir), false);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});
