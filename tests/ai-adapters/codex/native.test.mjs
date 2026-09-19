import assert from "node:assert/strict";
import test from "node:test";
import { VerifiedProviderSession } from "../../../packages/ai-contract/dist/session.js";
import {
  unwrap,
  fixtureSession,
} from "../../../packages/ai-contract/dist/testing/index.js";
import {
  nativeFixture,
  budget,
  conversation,
  prompt,
  reply,
} from "./helpers.mjs";

test(
  "fixed app-server: steer shares one turn and retains both client IDs",
  { timeout: 30000 },
  async (t) => {
    let release, entered;
    const received = new Promise((resolve) => {
      entered = resolve;
    });
    const s = await nativeFixture(t, {
      handleModel: async (_body, response, index) => {
        if (index === 1) {
          entered();
          await new Promise((resolve) => {
            release = resolve;
          });
        }
        reply(response);
      },
    });
    const port = s.make(),
      admitted = unwrap(
        await VerifiedProviderSession.open(port, s.configuration, budget()),
      );
    const start = prompt(admitted.binding, "start");
    const submitted = await port.submit(
      admitted.binding,
      start.command,
      start.attempt,
      budget(),
    );
    assert.equal(submitted.certainty, "submitted");
    await received;
    const steer = prompt(admitted.binding, "steer");
    steer.command.input = {
      type: "prompt",
      policy: "steer",
      text: "Please finish now",
      targetRunId: submitted.binding.nativeRunId,
    };
    steer.attempt.nativeRunId = submitted.binding.nativeRunId;
    const redirected = await port.submit(
      submitted.binding,
      steer.command,
      steer.attempt,
      budget(),
    );
    assert.equal(redirected.certainty, "submitted");
    assert.equal(redirected.binding.nativeRunId, submitted.binding.nativeRunId);
    release();
    const terminals = [];
    for await (const event of port.observe(admitted.binding, budget())) {
      if (event.body?.type === "terminal") terminals.push(event);
      if (terminals.length === 2) break;
    }
    assert.deepEqual(
      terminals.map((event) => event.commandId),
      ["start", "steer"],
    );
    const history = unwrap(await port.readHistory(admitted.binding, budget()));
    assert.equal(history.length, 1);
    assert.deepEqual(
      history[0].items
        .filter((item) => item.type === "userMessage")
        .map((item) => item.clientId),
      [start.attempt.attemptId, steer.attempt.attemptId],
    );
  },
);

test(
  "fixed app-server: interrupt remains a request until the cancelled native terminal",
  { timeout: 30000 },
  async (t) => {
    let entered;
    const received = new Promise((resolve) => {
      entered = resolve;
    });
    const s = await nativeFixture(t, {
      handleModel: async () => {
        entered();
      },
    });
    const port = s.make(),
      admitted = unwrap(
        await VerifiedProviderSession.open(port, s.configuration, budget()),
      );
    const start = prompt(admitted.binding, "cancel-me");
    const sent = await port.submit(
      admitted.binding,
      start.command,
      start.attempt,
      budget(),
    );
    assert.equal(sent.certainty, "submitted");
    await received;
    const result = unwrap(
      await port.cancel(
        sent.binding,
        {
          ...start.command,
          commandId: "cancel-request",
          input: {
            type: "cancel",
            targetCommandId: "cancel-me",
            generation: admitted.binding.generation,
            nativeRunId: sent.binding.nativeRunId,
          },
        },
        budget(),
      ),
    );
    assert.equal(result, "request_only");
    let terminal;
    for await (const event of port.observe(admitted.binding, budget()))
      if (event.body?.type === "terminal") {
        terminal = event;
        break;
      }
    assert.equal(terminal?.body.outcome, "cancelled");
  },
);

test(
  "fixed app-server: model HTTP failure cannot become successful completion",
  { timeout: 30000 },
  async (t) => {
    const s = await nativeFixture(t, {
      handleModel: async (_body, response) => {
        response.writeHead(401, { "content-type": "application/json" });
        response.end(
          JSON.stringify({ error: { message: "fixture auth rejected" } }),
        );
      },
    });
    const port = s.make(),
      admitted = unwrap(
        await VerifiedProviderSession.open(port, s.configuration, budget()),
      );
    const start = prompt(admitted.binding, "http-error");
    assert.equal(
      (
        await port.submit(
          admitted.binding,
          start.command,
          start.attempt,
          budget(),
        )
      ).certainty,
      "submitted",
    );
    let terminal;
    for await (const event of port.observe(admitted.binding, budget()))
      if (event.body?.type === "terminal") {
        terminal = event;
        break;
      }
    assert.equal(terminal?.body.outcome, "failed");
  },
);

test(
  "fixed app-server: isolated text stream, native history, cold restore and explicit terminal fork",
  { timeout: 60000 },
  async (t) => {
    const s = await nativeFixture(t),
      port = s.make();
    const admitted = unwrap(
      await VerifiedProviderSession.open(port, s.configuration, budget()),
    );
    const first = await conversation(port, admitted, "first");
    assert.equal(s.requests.length, 1);
    assert.deepEqual(s.requests[0].tools ?? [], []);
    const history = unwrap(await port.readHistory(admitted.binding, budget()));
    assert.ok(
      history[0].items.some(
        (item) =>
          item.type === "userMessage" &&
          item.clientId === first.attempt.attemptId,
      ),
    );
    s.lineage.set(admitted.binding.nativeThreadId, {
      nativeSessionId: admitted.binding.nativeSessionId,
      nativeThreadId: admitted.binding.nativeThreadId,
    });
    const previous = {
      ...fixtureSession(),
      namespace: s.configuration.namespace,
      binding: first.submission.binding,
      capabilities: admitted.capabilities,
    };
    unwrap(await port.close(budget()));
    const restoredPort = s.make();
    const restored = unwrap(
      await VerifiedProviderSession.restore(
        restoredPort,
        previous,
        s.configuration,
        budget(),
      ),
    );
    assert.notEqual(restored.binding.generation, admitted.binding.generation);
    assert.equal(
      restored.binding.nativeThreadId,
      admitted.binding.nativeThreadId,
    );
    const fork = await restoredPort.fork(
      restored.binding,
      history[0].id,
      {
        ...s.configuration,
        namespace: { ...s.configuration.namespace, sessionId: "fork-1" },
      },
      budget(),
    );
    assert.equal(fork.certainty, "created");
    s.ports.push(fork.port);
    assert.notEqual(
      fork.session.binding.nativeSessionId,
      admitted.binding.nativeSessionId,
    );
    assert.notEqual(
      fork.session.binding.nativeThreadId,
      admitted.binding.nativeThreadId,
    );
    assert.equal(fork.source.nativeThreadId, admitted.binding.nativeThreadId);
    await conversation(restoredPort, restored, "second");
    assert.equal(s.requests.length, 2);
  },
);

test(
  "fixed app-server: required HTTP MCP connects with the exact inert resource and proposal catalog",
  { timeout: 40000 },
  async (t) => {
    const s = await nativeFixture(t, { controlled: true }),
      port = s.make();
    const admitted = unwrap(
      await VerifiedProviderSession.open(port, s.configuration, budget()),
    );
    await conversation(port, admitted, "controlled");
    const names = s.requests[0].tools
      .flatMap((tool) =>
        tool.type === "namespace"
          ? tool.tools.map((child) => `${tool.name}__${child.name}`)
          : [tool.name],
      )
      .sort();
    assert.deepEqual(
      names,
      [
        "list_mcp_resource_templates",
        "list_mcp_resources",
        "mcp__rss_host__propose",
        "read_mcp_resource",
      ].sort(),
    );
  },
);
