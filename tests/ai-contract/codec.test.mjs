import assert from "node:assert/strict";
import { test } from "node:test";
import {
  decode,
  fingerprint,
  ContractError,
} from "../../packages/ai-contract/dist/index.js";
const limits = {
  maxBytes: 65536,
  maxTextBytes: 4096,
  maxDepth: 32,
  maxNodes: 4096,
};
const command = {
  schemaVersion: 5,
  kind: "command",
  sessionId: "s1",
  commandId: "c1",
  expiresAtMs: 1000,
  input: { type: "prompt", text: "hello", policy: "queue_next" },
};
test("V3 command round trips without a Rust, UI or provider runtime", () => {
  assert.deepEqual(decode(JSON.stringify(command), limits), command);
});
test("old formats, duplicate keys and excess authority fields fail closed", () => {
  for (const raw of [
    JSON.stringify({ ...command, schemaVersion: 1 }),
    JSON.stringify({ ...command, approved: true }),
    JSON.stringify(command).replace('"c1"', '"c1","commandId":"c2"'),
  ])
    assert.throws(() => decode(raw, limits), ContractError);
});
test("private control and execution provenance use the same closed generated schema", () => {
  const context = {
    schemaVersion: 5,
    kind: "userContext",
    user: {
      schemaVersion: 5,
      kind: "testUser",
      userId: "alice",
      displayName: "Alice",
      nameKey: "alice",
    },
    generation: "generation-1",
  };
  const suspend = {
    schemaVersion: 5,
    kind: "nativeCall",
    id: 1,
    method: "suspend",
    data: { context },
  };
  assert.deepEqual(decode(JSON.stringify(suspend), limits), suspend);
  const origin = {
    schemaVersion: 5,
    kind: "executionOrigin",
    namespace: {
      tenantId: "test-users",
      principalId: "alice",
      authorityId: "desktop-fixture",
      sessionId: "session-1",
    },
    operationId: "operation-1",
    provider: "codex",
    config: { id: "connection-1", revision: "1" },
  };
  assert.deepEqual(decode(JSON.stringify(origin), limits), origin);
  for (const invalid of [
    { ...suspend, method: "unknown" },
    { ...suspend, data: { context, generation: "duplicate-authority" } },
    { ...origin, provider: "arbitrary" },
  ])
    assert.throws(() => decode(JSON.stringify(invalid), limits), ContractError);
});
test("canonical command identity ignores key order but binds all content", () => {
  assert.equal(
    fingerprint(command, limits),
    fingerprint(
      {
        ...command,
        input: { policy: "queue_next", text: "hello", type: "prompt" },
      },
      limits,
    ),
  );
  assert.notEqual(
    fingerprint(command, limits),
    fingerprint({ ...command, expiresAtMs: 1001 }, limits),
  );
});
import {
  fixtures,
  fixtureLimits,
} from "../../packages/ai-contract/dist/testing/index.js";
test("shared golden covers every record and identical rejection diagnostics", () => {
  for (const value of fixtures.valid)
    assert.deepEqual(decode(JSON.stringify(value), fixtureLimits), value);
  for (const c of fixtures.invalid)
    assert.throws(
      () => decode(c.raw, { ...fixtureLimits, ...c.limits }),
      (e) => e instanceof ContractError && e.code === c.code,
      c.name,
    );
  assert.equal(
    fingerprint(fixtures.valid[0], fixtureLimits),
    fixtures.commandHash,
  );
});
