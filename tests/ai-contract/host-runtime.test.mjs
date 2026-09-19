import assert from "node:assert/strict";
import { test } from "node:test";
import { readFileSync } from "node:fs";
import {
  ScriptedProvider,
  fixtureSession,
} from "../../packages/ai-contract/dist/testing/index.js";
import { decode } from "../../packages/ai-contract/dist/index.js";

test("provider commands share an attempt-bound dispatch port", () => {
  assert.equal(typeof new ScriptedProvider().dispatch, "function");
});

test("recovery unavailability belongs to the durable Session", () => {
  const session = { ...fixtureSession(), status: "recovery_required" };
  assert.equal(
    decode(JSON.stringify(session), {
      maxBytes: 262144,
      maxTextBytes: 131072,
      maxDepth: 32,
      maxNodes: 16384,
    }).status,
    "recovery_required",
  );
});

test("provider capabilities do not own the durable Host queue", () => {
  const schema = JSON.parse(
    readFileSync(
      new URL(
        "../../packages/ai-contract/schema/runtime.schema.json",
        import.meta.url,
      ),
    ),
  );
  assert.equal("queue" in schema.$defs.Capabilities.properties, false);
});
