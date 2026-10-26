// Explicit MCP peer fixture: exercise the production Host's identity and recovery mapping.
import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { readFile } from "node:fs/promises";
import { PassThrough } from "node:stream";
import { test } from "node:test";
import {
  connectExecution,
  ParentTransport,
} from "../../apps/ai-host/dist/execution.js";
import { fixtureSession } from "../../packages/ai-contract/dist/testing/index.js";
const require = createRequire(
  new URL("../../apps/ai-host/package.json", import.meta.url),
);
const { Server } = require("@modelcontextprotocol/sdk/server/index.js");
const {
  ListToolsRequestSchema,
  CallToolRequestSchema,
} = require("@modelcontextprotocol/sdk/types.js");
const tools = JSON.parse(
  await readFile(
    new URL(
      "../../packages/execution-bindings/src/tools.json",
      import.meta.url,
    ),
    "utf8",
  ),
);
const budget = () => ({
  timeoutMs: 2000,
  signal: new AbortController().signal,
});

test("query response loss reconciles the Host-owned original ID and never replays submit or falsely confirms cancellation", async (t) => {
  const incoming = new PassThrough(),
    outgoing = new PassThrough();
  const peer = new Server(
    { name: "explicit-query-fixture", version: "1" },
    { capabilities: { tools: {} } },
  );
  const calls = [];
  let cancelled = false;
  peer.setRequestHandler(ListToolsRequestSchema, async ({ params }) => {
    const start = Number(params?.cursor ?? 0),
      end = Math.min(start + 3, tools.length);
    return {
      tools: tools.slice(start, end),
      ...(end < tools.length ? { nextCursor: String(end) } : {}),
    };
  });
  peer.setRequestHandler(CallToolRequestSchema, async ({ params }) => {
    calls.push(params);
    const failure = [
      "execution_query_submit",
      "execution_task_control",
    ].includes(params.name);
    const value = failure
      ? { status: "error", error: { code: "outcomeUnknown" } }
      : params.name === "execution_task_control_status"
        ? {
            status: "ok",
            result: {
              id: params.arguments.id,
              disposition: cancelled ? "confirmed" : "unknown",
              created: null,
            },
          }
        : {
            status: "ok",
            result: {
              kind: "status",
              value: {
                request: params.arguments.request,
                query: { id: "native:fixture", revision: "a".repeat(64) },
                preparation: null,
                phase: "running",
                cancelRequested: cancelled,
              },
            },
          };
    return { content: [], structuredContent: value, isError: failure };
  });
  await peer.connect(new ParentTransport(incoming, outgoing));
  const connection = await connectExecution(outgoing, incoming, async () => ({
    binding: { provider: "codex", config: { id: "fixture", revision: "1" } },
    userGeneration: "fixture-generation",
    contextBinding: "fixture-binding",
  }));
  t.after(async () => {
    await connection.close();
    await peer.close();
    incoming.destroy();
    outgoing.destroy();
  });
  const namespace = fixtureSession().namespace;
  const proposal = {
    callId: "fixture-call",
    name: "execution_query_submit",
    arguments: {
      request: "model-chosen-id",
      query: { id: "native:fixture", revision: "a".repeat(64) },
      parameters: {},
    },
  };
  const prepared = connection.router.prepare(namespace, proposal, "command");
  assert.equal(prepared.ok, true);
  const request = {
    namespace,
    commandId: "command",
    body: { type: "delivery_requested", ...prepared.value, proposal },
  };
  assert.equal((await connection.router.send(request, budget())).ok, false);
  assert.notEqual(calls[0].arguments.request, "model-chosen-id");
  assert.equal(calls[0].arguments.request, prepared.value.operationId);
  assert.equal(
    (await connection.router.reconcile(request, budget())).value.state,
    "committed",
  );
  assert.equal(calls[1].name, "execution_query_status");
  assert.equal(calls[1].arguments.request, prepared.value.operationId);
  assert.equal(
    calls.filter((c) => c.name === "execution_query_submit").length,
    1,
  );
  const cancelProposal = {
    name: "execution_task_control",
    arguments: {
      reference: { kind: "request", request: prepared.value.operationId },
      id: "cancel-original",
      scope: "fixture-scope",
      expectedRevision: { owner: 0, transfers: [] },
      action: "cancel",
    },
  };
  const cancellation = connection.router.prepare(
    namespace,
    cancelProposal,
    "cancel-command",
  );
  assert.equal(cancellation.ok, true);
  const cancelRequest = {
    ...request,
    commandId: "cancel-command",
    body: {
      type: "delivery_requested",
      ...cancellation.value,
      proposal: cancelProposal,
    },
  };
  assert.equal(
    (await connection.router.reconcile(cancelRequest, budget())).value.state,
    "unknown",
  );
  assert.equal(calls.at(-1).name, "execution_task_control_status");
  assert.equal(calls.at(-1).arguments.id, "cancel-original");
  assert.equal(
    calls.filter((c) => c.name === "execution_task_control").length,
    0,
  );
  cancelled = true;
  assert.equal(
    (await connection.router.reconcile(cancelRequest, budget())).value.state,
    "committed",
  );
});
