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
  MemorySessionStore,
  seedSurface,
  surfaceCommit,
  emptyCommit,
  acceptance,
  unwrap,
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
test("official A2UI action binds exact surface/run/revision; payload claims grant nothing", async () => {
  const store = new MemorySessionStore();
  const { surface } = await seedSurface(store);
  const meta = {
    ...fixtures.valid.find((v) => v.kind === "surfaceAction"),
    interactionId: surface.interactionId,
    surfaceRevision: surface.revision,
  };
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
  const result = await resolveSurfaceAction(
    store,
    fixtureCaller,
    meta,
    action,
    fixtureLimits,
  );
  assert.equal(
    (
      await resolveSurfaceAction(
        store,
        fixtureCaller,
        fixtures.valid[0],
        action,
        fixtureLimits,
      )
    ).error.code,
    "invalid_input",
  );
  assert.equal(
    (
      await resolveSurfaceAction(
        { surface: async () => ({ ok: true, value: fixtures.valid[0] }) },
        fixtureCaller,
        meta,
        action,
        fixtureLimits,
      )
    ).error.code,
    "invalid_input",
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
      (
        await resolveSurfaceAction(
          store,
          fixtureCaller,
          { ...meta, ...patch },
          action,
          fixtureLimits,
        )
      ).error.code,
      "stale_binding",
    );
  assert.equal(
    (
      await resolveSurfaceAction(
        store,
        { ...fixtureCaller, tenantId: "other" },
        meta,
        action,
        fixtureLimits,
      )
    ).error.code,
    "session_gone",
  );
});
test("negotiation permits basic ACP without A2UI and rejects unselected versions", () => {
  const host = new FakeHost();
  const basic = {
    contractVersion: 5,
    acp: 1,
    durableReceipts: false,
    cursorAttach: false,
  };
  assert.deepEqual(host.negotiate(basic), { ok: true, value: basic });
  assert.equal(host.negotiate({ ...basic, contractVersion: 1 }).ok, false);
});

test("A2UI constructed inputs are bounded before upstream validation", async () => {
  const store = new MemorySessionStore();
  const { surface } = await seedSurface(store);
  const meta = {
    ...fixtures.valid.find((v) => v.kind === "surfaceAction"),
    interactionId: surface.interactionId,
    surfaceRevision: surface.revision,
  };
  const circular = {};
  circular.self = circular;
  const accessor = {};
  Object.defineProperty(accessor, "secret", {
    enumerable: true,
    get() {
      throw new Error("accessor executed");
    },
  });
  let deep = {};
  for (let i = 0; i < 80; i++) deep = { nested: deep };
  for (const context of [
    circular,
    accessor,
    deep,
    { items: Array(17000).fill(0) },
    { text: "x".repeat(300000) },
  ]) {
    const action = {
      version: "v0.9.1",
      action: {
        name: "respond",
        surfaceId: "surface-1",
        sourceComponentId: "button-1",
        timestamp: "2026-09-18T00:00:00Z",
        context,
      },
    };
    assert.equal(
      (
        await resolveSurfaceAction(
          store,
          fixtureCaller,
          meta,
          action,
          fixtureLimits,
        )
      ).error.code,
      "invalid_input",
    );
  }
});
test("fixed upstream surface lifecycle and basic catalog schemas form a complete offline closure", () => {
  const read = (name) =>
    JSON.parse(
      readFileSync(
        new URL(
          `../../packages/ai-contract/schema/upstream/a2ui/${name}.json`,
          import.meta.url,
        ),
        "utf8",
      ),
    );
  const validator = new Ajv2020({ strict: false, validateFormats: false })
    .addSchema(read("common_types"))
    .addSchema(
      read("catalog"),
      "https://a2ui.org/specification/v0_9/catalog.json",
    )
    .compile(read("server_to_client"));
  for (const value of fixtures.a2uiServer)
    assert.equal(validator(value), true, JSON.stringify(validator.errors));
  assert.equal(validator({ version: "v0.9.1", deleteSurface: {} }), false);
  assert.equal(
    validator({
      version: "v0.9.1",
      updateComponents: {
        surfaceId: "surface-1",
        components: [{ id: "root", component: "InventedComponent" }],
      },
    }),
    false,
  );
});

test("surface deleted between resolution and response acceptance rejects the action", async () => {
  const store = new MemorySessionStore();
  const seeded = await seedSurface(store),
    { surface } = seeded;
  const meta = {
    ...fixtures.valid.find((v) => v.kind === "surfaceAction"),
    interactionId: surface.interactionId,
    surfaceRevision: 0,
  };
  const action = {
    version: "v0.9.1",
    action: {
      name: surface.eventName,
      surfaceId: surface.surfaceId,
      sourceComponentId: surface.sourceComponentId,
      timestamp: "2026-09-18T00:00:00Z",
      context: { choice: "allow" },
    },
  };
  const resolved = unwrap(
    await resolveSurfaceAction(
      store,
      fixtureCaller,
      meta,
      action,
      fixtureLimits,
    ),
  );
  unwrap(
    await store.commit(
      surfaceCommit(
        seeded.session,
        { ...surface, revision: 1, status: "deleted" },
        seeded.interaction,
      ),
    ),
  );
  assert.equal(
    (
      await resolveSurfaceAction(
        store,
        fixtureCaller,
        meta,
        action,
        fixtureLimits,
      )
    ).error.code,
    "unavailable",
  );
  const head = unwrap(await store.session(seeded.session.namespace));
  const answer = {
    ...seeded.answer,
    input: { ...seeded.answer.input, surface: resolved.surface },
  };
  assert.equal((await store.accept(acceptance(head, answer))).ok, false);
});
