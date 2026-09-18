import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { test } from "node:test";
import { resolveSurfaceAction } from "../../packages/ai-contract/dist/index.js";
import {
  fixtureCaller,
  fixtures,
  fixtureLimits,
  FakeHost,
} from "../../packages/ai-contract/dist/testing/index.js";
const require = createRequire(
  new URL("../../packages/ai-contract/package.json", import.meta.url),
);
const { Ajv2020 } = require("ajv/dist/2020.js");
const acp = require("@agentclientprotocol/sdk/schema/schema.json");
const validator = new Ajv2020({
  strict: false,
  validateFormats: false,
}).compile({
  $schema: acp.$schema,
  $defs: acp.$defs,
  $ref: "#/$defs/PromptResponse",
});
test("published ACP PromptResponse accepts final stop reason, never a durable receipt", () => {
  assert.equal(validator({ stopReason: "end_turn" }), true);
  assert.equal(validator({ stopReason: "cancelled" }), true);
  assert.equal(
    validator(fixtures.valid.find((v) => v.kind === "receipt")),
    false,
  );
  assert.equal(validator({ stopReason: "accepted" }), false);
});
test("official A2UI action binds exact surface/run/revision; payload claims grant nothing", () => {
  const surface = fixtures.valid.find((v) => v.kind === "surface"),
    meta = fixtures.valid.find((v) => v.kind === "surfaceAction");
  const action = {
    version: "v0.9.1",
    action: {
      name: "respond",
      surfaceId: "surface-1",
      sourceComponentId: "button-1",
      timestamp: "2026-09-18T00:00:00Z",
      context: { approved: true, actor: "model-claim" },
    },
  };
  const result = resolveSurfaceAction(
    fixtureCaller,
    surface,
    meta,
    action,
    fixtureLimits,
  );
  assert.equal(result.ok, true);
  assert.deepEqual(result.value.answer, action.action.context);
  for (const patch of [
    { surfaceRevision: 2 },
    { generation: "stale" },
    { interactionId: "different" },
    { surfaceInstanceId: "different" },
    { nativeRunId: "different" },
  ])
    assert.equal(
      resolveSurfaceAction(
        fixtureCaller,
        surface,
        { ...meta, ...patch },
        action,
        fixtureLimits,
      ).error.code,
      "stale_binding",
    );
  assert.equal(
    resolveSurfaceAction(
      { ...fixtureCaller, tenantId: "other" },
      surface,
      meta,
      action,
      fixtureLimits,
    ).error.code,
    "permission_denied",
  );
});
test("negotiation permits basic ACP without A2UI and rejects unselected versions", () => {
  const host = new FakeHost();
  const basic = {
    contractVersion: 2,
    acp: 1,
    durableReceipts: false,
    cursorAttach: false,
  };
  assert.deepEqual(host.negotiate(basic), { ok: true, value: basic });
  assert.equal(host.negotiate({ ...basic, contractVersion: 1 }).ok, false);
});
