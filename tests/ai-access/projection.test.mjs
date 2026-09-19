import assert from "node:assert/strict";
import { test } from "node:test";
import {
  emptyView,
  applyUpdate,
} from "../../packages/ai-client/dist/projection.js";
import { fixtureSession } from "../../packages/ai-contract/dist/testing/index.js";
const session = fixtureSession();
const event = (sequence, body) => ({
  type: "event",
  event: {
    schemaVersion: 2,
    kind: "event",
    namespace: session.namespace,
    eventId: `e-${sequence}`,
    sequence,
    commandId: "c",
    generation: session.binding.generation,
    body,
  },
});
test("projection refuses gaps and cross-caller events, ignores duplicates, and preserves terminal against late delta", () => {
  const view = emptyView(session, 0);
  view.connection = "attached";
  applyUpdate(view, event(1, { type: "text", messageId: "m", text: "stable" }));
  applyUpdate(
    view,
    event(1, { type: "text", messageId: "m", text: "duplicate" }),
  );
  applyUpdate(view, event(2, { type: "terminal", outcome: "completed" }));
  applyUpdate(view, {
    type: "delta",
    commandId: "c",
    messageId: "m",
    generation: session.binding.generation,
    text: "late",
  });
  assert.equal(view.messages["c/m"].text, "stable");
  const foreign = event(3, { type: "text", messageId: "m", text: "foreign" });
  foreign.event.namespace = { ...session.namespace, principalId: "other" };
  assert.throws(() => applyUpdate(view, foreign));
  assert.equal(view.cursor, 2);
  applyUpdate(view, event(4, { type: "text", messageId: "m", text: "gap" }));
  assert.equal(view.connection, "resync_required");
  assert.equal(view.cursor, 2);
});
