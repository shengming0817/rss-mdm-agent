import assert from "node:assert/strict";
import { test } from "node:test";
import { createRequire } from "node:module";
import { createAccessService } from "../../packages/ai-access/dist/index.js";
import {
  RuntimeClient,
  localTransportPair,
} from "../../packages/ai-client/dist/index.js";
import { extension } from "../../packages/ai-contract/dist/index.js";
import {
  FakeHost,
  fixtureCaller,
  unwrap,
} from "../../packages/ai-contract/dist/testing/index.js";
const require = createRequire(
  new URL("../../packages/ai-access/package.json", import.meta.url),
);
const { client } = await import(require.resolve("@agentclientprotocol/sdk"));
const options = {
  provider: "fake",
  accountRef: "account",
  config: { id: "cfg", revision: "1" },
  profile: "conversation",
};
export async function until(read) {
  for (let i = 0; i < 200; i++) {
    const result = await read();
    if (result) return result;
    await new Promise((r) => setTimeout(r, 5));
  }
  throw new Error("condition timed out");
}
function setup(t, capabilities = {}) {
  const host = new FakeHost(undefined, { now: () => 0 }, capabilities);
  const service = createAccessService({
    host,
    sessionOptions: options,
    now: () => 0,
    timeoutMs: 5000,
  });
  t.after(() => service.close());
  return { host, service };
}
async function standard(
  t,
  service,
  onPermission = async () => ({ outcome: { outcome: "cancelled" } }),
) {
  const updates = [],
    [a, b] = localTransportPair();
  service.connect(a, fixtureCaller);
  const connection = client()
    .onNotification("session/update", ({ params }) => updates.push(params))
    .onRequest("session/request_permission", ({ params, signal }) =>
      onPermission(params, signal),
    )
    .connect(b);
  t.after(() => connection.close());
  await connection.agent.request("initialize", {
    protocolVersion: 1,
    clientCapabilities: {},
  });
  return { connection, updates, agent: connection.agent };
}
async function command(host, sessionId) {
  return until(async () => {
    const page = unwrap(
      await host.store.snapshotPage(
        { ...fixtureCaller, sessionId },
        { limit: 64 },
      ),
    );
    return page.commands.find((c) => c.command.input.type === "prompt")
      ?.command;
  });
}
test("standard ACP prompt waits for terminal, carries text/tool updates, and supports bidirectional permission", async (t) => {
  const { host, service } = setup(t);
  const { agent, updates } = await standard(t, service, async (request) => {
    assert.equal(request.toolCall.title, "Read selected file");
    return { outcome: { outcome: "selected", optionId: "allow" } };
  });
  const { sessionId } = await agent.request("session/new", {
    cwd: "/",
    mcpServers: [],
  });
  let ended = false;
  const pending = agent
    .request("session/prompt", {
      sessionId,
      prompt: [{ type: "text", text: "Hello" }],
    })
    .then((v) => {
      ended = true;
      return v;
    });
  const accepted = await command(host, sessionId);
  assert.equal(ended, false);
  unwrap(
    await host.advance(fixtureCaller, sessionId, accepted.commandId, [
      { type: "text", messageId: "answer", text: "Hello back" },
      {
        type: "tool_proposal",
        proposalId: "tool-1",
        name: "read",
        arguments: { path: "untrusted" },
      },
    ]),
  );
  const permission = await service.requestPermission(
    fixtureCaller,
    {
      sessionId,
      toolCall: {
        toolCallId: "tool-1",
        title: "Read selected file",
        status: "pending",
      },
      options: [
        { optionId: "allow", name: "Allow once", kind: "allow_once" },
        { optionId: "deny", name: "Reject once", kind: "reject_once" },
      ],
    },
    AbortSignal.timeout(1000),
  );
  assert.equal(permission.outcome.optionId, "allow");
  assert.equal(ended, false);
  unwrap(
    await host.advance(fixtureCaller, sessionId, accepted.commandId, [
      {
        type: "tool_result",
        proposalId: "tool-1",
        disposition: "returned",
        text: "Tool result",
      },
      { type: "terminal", outcome: "completed" },
    ]),
  );
  assert.deepEqual(await pending, { stopReason: "end_turn" });
  assert.deepEqual(
    updates.map((u) => u.update.sessionUpdate),
    ["agent_message_chunk", "tool_call", "tool_call_update"],
  );
  await assert.rejects(
    agent.request(extension.list, {
      schemaVersion: 4,
      kind: "listRequest",
      query: { limit: 2 },
    }),
    /unsupported_capability/,
  );
});
test("same connection separates sessions, cancel waits for native terminal, disconnect preserves accepted work", async (t) => {
  const { host, service } = setup(t);
  const { agent, connection } = await standard(t, service);
  const ids = await Promise.all(
    [1, 2].map(
      async () =>
        (await agent.request("session/new", { cwd: "/", mcpServers: [] }))
          .sessionId,
    ),
  );
  const turns = ids.map((sessionId) =>
    agent.request("session/prompt", {
      sessionId,
      prompt: [{ type: "text", text: sessionId }],
    }),
  );
  const commands = await Promise.all(ids.map((id) => command(host, id)));
  await agent.notify("session/cancel", { sessionId: ids[0] });
  await until(async () =>
    unwrap(
      await host.store.snapshotPage(
        { ...fixtureCaller, sessionId: ids[0] },
        { limit: 64 },
      ),
    ).commands.some((c) => c.command.input.type === "cancel"),
  );
  unwrap(
    await host.advance(fixtureCaller, ids[0], commands[0].commandId, [
      { type: "terminal", outcome: "cancelled" },
    ]),
  );
  assert.equal((await turns[0]).stopReason, "cancelled");
  const lost = assert.rejects(turns[1]);
  connection.close();
  await lost;
  const remaining = unwrap(
    await host.store.snapshotPage(
      { ...fixtureCaller, sessionId: ids[1] },
      { limit: 64 },
    ),
  );
  assert.equal(remaining.commands.length, 1);
  assert.equal(remaining.commands[0].state, "accepted");
});
test("product receipt, paged recovery and late delta use only the shared stable projection", async (t) => {
  const { host, service } = setup(t, {
    continuation: "same_process",
    steer: "supported",
  });
  const [a, b] = localTransportPair();
  service.connect(a, fixtureCaller);
  const runtime = new RuntimeClient(b);
  t.after(() => runtime.close());
  await runtime.initialize();
  const view = await runtime.createSession(),
    id = view.namespace.sessionId;
  const prompt = {
    schemaVersion: 4,
    kind: "command",
    sessionId: id,
    commandId: "product-1",
    expiresAtMs: 1000,
    input: { type: "prompt", policy: "queue_next", text: "test" },
  };
  const receipt = await runtime.submit(prompt);
  assert.equal(receipt.kind, "receipt");
  unwrap(
    await host.advance(fixtureCaller, id, prompt.commandId, [
      { type: "text", messageId: "m", text: "stable" },
      { type: "terminal", outcome: "max_tokens" },
    ]),
  );
  await until(
    () =>
      runtime.getSession(id).commands[prompt.commandId]?.state === "terminal",
  );
  const s = unwrap(
    await host.store.session({ ...fixtureCaller, sessionId: id }),
  );
  assert.equal(
    (
      await host.publishDelta(fixtureCaller, id, {
        type: "delta",
        binding: s.binding,
        commandId: prompt.commandId,
        attemptId: `attempt-${prompt.commandId}`,
        messageId: "m",
        text: "late",
      })
    ).ok,
    false,
    "Host rejects deltas from an already terminal attempt",
  );
  const recovered = await runtime.restore(id, 1);
  assert.equal(recovered.commands[prompt.commandId].outcome, "max_tokens");
  assert.equal(
    recovered.messages[JSON.stringify([prompt.commandId, "m"])].text,
    "stable",
  );
  assert.equal((await runtime.resume(id)).namespace.sessionId, id);
  assert.equal((await runtime.listSessions({ limit: 1 })).items.length, 1);
});

test("standard load replays history after disconnect and preserves precise stop reasons", async (t) => {
  const { host, service } = setup(t);
  const first = await standard(t, service);
  const { sessionId } = await first.agent.request("session/new", {
    cwd: "/",
    mcpServers: [],
  });
  for (const [outcome, stopReason] of [
    ["refused", "refusal"],
    ["max_tokens", "max_tokens"],
    ["max_turn_requests", "max_turn_requests"],
    ["failed", null],
  ]) {
    const turn = first.agent.request("session/prompt", {
      sessionId,
      prompt: [
        {
          type: "resource_link",
          name: "Document",
          uri: "file:///unread-secret",
        },
      ],
    });
    const observed = turn.then(
      (value) => ({ value }),
      (error) => ({ error }),
    );
    const pending = await until(async () =>
      unwrap(
        await host.store.snapshotPage(
          { ...fixtureCaller, sessionId },
          { limit: 64 },
        ),
      ).commands.find(
        (c) => c.state !== "terminal" && c.command.input.type === "prompt",
      ),
    );
    assert.equal(
      pending.command.input.text,
      "[Resource reference: Document; URI: file:///unread-secret]",
    );
    unwrap(
      await host.advance(fixtureCaller, sessionId, pending.command.commandId, [
        { type: "text", messageId: outcome, text: outcome },
        { type: "terminal", outcome },
      ]),
    );
    const result = await observed;
    if (stopReason) assert.equal(result.value.stopReason, stopReason);
    else assert.ok(result.error);
  }
  first.connection.close();
  const second = await standard(t, service);
  await second.agent.request("session/load", {
    sessionId,
    cwd: "/",
    mcpServers: [],
  });
  assert.deepEqual(
    second.updates.map((v) => v.update.content.text),
    ["refused", "max_tokens", "max_turn_requests", "failed"],
  );
  assert.equal(
    (await second.agent.request("session/list", {})).sessions[0].sessionId,
    sessionId,
  );
  await assert.rejects(
    second.agent.request("session/resume", {
      sessionId,
      cwd: "/",
      mcpServers: [],
    }),
    /unsupported_capability/,
  );
});

test("permission first response cancels other deliveries; expired live callback cannot be recovered", async (t) => {
  const { service } = setup(t);
  let cancelled = false;
  const first = await standard(t, service, async () => ({
    outcome: { outcome: "selected", optionId: "deny" },
  }));
  const { sessionId } = await first.agent.request("session/new", {
    cwd: "/",
    mcpServers: [],
  });
  const second = await standard(t, service, async (_request, signal) => {
    await new Promise((resolve) => {
      if (signal.aborted) resolve();
      else signal.addEventListener("abort", resolve, { once: true });
    });
    cancelled = true;
    return { outcome: { outcome: "cancelled" } };
  });
  await second.agent.request("session/load", {
    sessionId,
    cwd: "/",
    mcpServers: [],
  });
  const request = {
    sessionId,
    toolCall: { toolCallId: "p", title: "Permission question" },
    options: [{ optionId: "deny", kind: "reject_once", name: "Reject" }],
  };
  const result = await service.requestPermission(
    fixtureCaller,
    request,
    AbortSignal.timeout(1000),
  );
  assert.equal(result.outcome.optionId, "deny");
  await until(() => cancelled);
  const ended = new AbortController();
  ended.abort();
  assert.equal(
    (await service.requestPermission(fixtureCaller, request, ended.signal))
      .outcome.outcome,
    "cancelled",
  );
});
