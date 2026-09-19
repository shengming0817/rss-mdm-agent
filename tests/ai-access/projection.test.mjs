import assert from "node:assert/strict";
import { test } from "node:test";
import {
  emptyView as blankView,
  applyUpdate,
  restoreSnapshot,
} from "../../packages/ai-client/dist/projection.js";
import {
  fixtureSession,
  fixtureCommand,
} from "../../packages/ai-contract/dist/testing/index.js";
const session = fixtureSession();
const event = (sequence, body) => ({
  type: "event",
  event: {
    schemaVersion: 3,
    kind: "event",
    namespace: session.namespace,
    eventId: `e-${sequence}`,
    sequence: sequence + 1,
    commandId: "c",
    attemptId: "attempt-c",
    generation: session.binding.generation,
    body,
  },
});
function emptyView(session) {
  const view = blankView(session, 0);
  applyUpdate(
    view,
    event(0, { type: "command_accepted", command: fixtureCommand("c") }),
  );
  return view;
}
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
  assert.equal(view.messages[JSON.stringify(["c", "m"])].text, "stable");
  const foreign = event(3, { type: "text", messageId: "m", text: "foreign" });
  foreign.event.namespace = { ...session.namespace, principalId: "other" };
  assert.throws(() => applyUpdate(view, foreign));
  assert.equal(view.cursor, 3);
  applyUpdate(view, event(4, { type: "text", messageId: "m", text: "gap" }));
  assert.equal(view.connection, "resync_required");
  assert.equal(view.cursor, 3);
});

test("public view has one cursor and no stale copy of authoritative Session metadata", () => {
  const view = emptyView(session, 0);
  assert.equal("session" in view, false);
  assert.deepEqual(view.namespace, session.namespace);
  assert.equal(view.generation, session.binding.generation);
  applyUpdate(view, event(1, { type: "status", state: "running" }));
  applyUpdate(view, event(2, { type: "terminal", outcome: "completed" }));
  assert.equal(view.cursor, 3);
});

test("recovery invalidation keeps durable text, clears transient text, and rejects late deltas", () => {
  const view = emptyView(session, 0);
  const failure = { code: "stale_binding", retry: "never" };
  const delta = {
    type: "delta",
    commandId: "c",
    messageId: "transient",
    generation: session.binding.generation,
    text: "uncommitted",
  };
  applyUpdate(view, event(1, { type: "status", state: "running" }));
  applyUpdate(
    view,
    event(2, { type: "text", messageId: "saved", text: "durable" }),
  );
  applyUpdate(view, delta);
  assert.equal(
    view.messages[JSON.stringify(["c", "transient"])].text,
    "uncommitted",
  );
  applyUpdate(view, event(3, { type: "invalidated", failure }));
  applyUpdate(view, { ...delta, text: "late" });
  applyUpdate(view, event(4, { type: "status", state: "running" }));
  assert.equal(view.commands.c.state, "invalidated");
  assert.deepEqual(view.commands.c.failure, failure);
  assert.deepEqual(Object.keys(view.messages), [
    JSON.stringify(["c", "saved"]),
  ]);
  assert.equal(view.messages[JSON.stringify(["c", "saved"])].text, "durable");
  assert.equal(view.cursor, 5);
});

test("tool proposals and results use the same stable reducer as snapshot replay", () => {
  const view = emptyView(session, 0);
  applyUpdate(
    view,
    event(1, {
      type: "tool_proposal",
      proposalId: "tool",
      name: "read",
      arguments: { path: "untrusted" },
    }),
  );
  assert.equal(view.tools[JSON.stringify(["c", "tool"])].status, "pending");
  applyUpdate(
    view,
    event(2, {
      type: "tool_result",
      proposalId: "tool",
      disposition: "returned",
      text: "content",
    }),
  );
  assert.deepEqual(view.tools[JSON.stringify(["c", "tool"])], {
    commandId: "c",
    proposalId: "tool",
    name: "read",
    arguments: { path: "untrusted" },
    status: "completed",
    result: { disposition: "returned", text: "content" },
  });
});

test("snapshot session health is independent of attachment and historical recovery events", () => {
  for (const status of ["active", "recovery_required", "retired"]) {
    const current = { ...session, status };
    const view = emptyView(current, 1);
    restoreSnapshot(view, [
      {
        session: current,
        events: [
          {
            ...event(1, { type: "session_recovery_unavailable" }).event,
            commandId: undefined,
          },
        ],
        commands: [],
        interactions: [],
        surfaces: [],
      },
    ]);
    view.connection = "attached";
    assert.equal(view.sessionStatus, status);
  }
  const live = emptyView(session, 0);
  live.connection = "attached";
  applyUpdate(live, event(1, { type: "session_recovery_unavailable" }));
  assert.equal(live.sessionStatus, "recovery_required");
});

test("slash-bearing command/block identities cannot collide in messages, tools or timeline", () => {
  const view = blankView(session, 0);
  let sequence = 0;
  const push = (commandId, body) =>
    applyUpdate(view, {
      type: "event",
      event: {
        schemaVersion: 3,
        kind: "event",
        namespace: session.namespace,
        eventId: `e-${++sequence}`,
        sequence,
        commandId,
        generation: session.binding.generation,
        body,
      },
    });
  for (const [id, block] of [
    ["a/b", "c"],
    ["a", "b/c"],
  ]) {
    push(id, { type: "command_accepted", command: fixtureCommand(id) });
    push(id, { type: "text", messageId: block, text: id });
    push(id, {
      type: "tool_proposal",
      proposalId: block,
      name: id,
      arguments: {},
    });
  }
  assert.equal(Object.keys(view.messages).length, 2);
  assert.equal(Object.keys(view.tools).length, 2);
  assert.equal(view.timeline.length, 6);
  assert.deepEqual(
    Object.values(view.messages).map((m) => m.text),
    ["a/b", "a"],
  );
});

test("cancellation projection retains unsupported, request-only and already-terminal distinctions", () => {
  const view = emptyView(session, 0);
  for (const [index, confirmation] of [
    "unsupported",
    "request_only",
    "already_terminal",
  ].entries()) {
    applyUpdate(
      view,
      event(index + 1, { type: "cancel_dispatched", confirmation }),
    );
    assert.equal(view.commands.c.cancelDispatched, confirmation);
    assert.equal(view.commands.c.state, "accepted");
  }
});
