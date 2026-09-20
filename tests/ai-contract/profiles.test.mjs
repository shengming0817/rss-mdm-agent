import assert from "node:assert/strict";
import test from "node:test";
import { decode as parse } from "../../packages/ai-contract/dist/index.js";

const decode = (raw) =>
  parse(raw, {
    maxBytes: 262144,
    maxTextBytes: 131072,
    maxDepth: 32,
    maxNodes: 16384,
  });
const session = {
  schemaVersion: 5,
  kind: "session",
  namespace: {
    tenantId: "test",
    authorityId: "desktop",
    principalId: "alice",
    sessionId: "chat",
  },
  revision: 0,
  lastSequence: 0,
  status: "active",
  stages: [],
};
test("a product session exists without credentials or a provider context", () => {
  assert.deepEqual(decode(JSON.stringify(session)), session);
  assert.throws(() => decode(JSON.stringify({ ...session, schemaVersion: 4 })));
  assert.throws(() => decode(JSON.stringify({ ...session, binding: {} })));
});
test("connection summaries cannot carry secrets or an owner supplied by a page", () => {
  const row = {
    schemaVersion: 5,
    kind: "connection",
    connectionId: "work",
    name: "Work",
    provider: "codex",
    configRevision: 1,
    credentialRevision: 1,
    accountRef: "account",
    profile: "conversation",
    status: "unverified",
    source: {
      type: "custom_api",
      apiUrl: "https://api.example.test/v1",
      model: "model",
    },
    credentialRef: "opaque-ref",
  };
  assert.deepEqual(decode(JSON.stringify(row)), row);
  for (const bad of [
    { apiKey: "secret" },
    { principalId: "bob" },
    { credentialPath: "/tmp/key" },
  ])
    assert.throws(() => decode(JSON.stringify({ ...row, ...bad })));
});
