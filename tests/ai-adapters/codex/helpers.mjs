import assert from "node:assert/strict";
import { createServer } from "node:http";
import { mkdtemp, mkdir, realpath, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createCodexAdapter } from "../../../packages/ai-adapters/codex/dist/index.js";
import { VerifiedProviderSession } from "../../../packages/ai-contract/dist/session.js";
import {
  unwrap,
  fixtureSession,
} from "../../../packages/ai-contract/dist/testing/index.js";

export const budget = () => ({
  timeoutMs: 20000,
  signal: new AbortController().signal,
});
export function reply(response, text = "fixture reply") {
  response.writeHead(200, { "content-type": "text/event-stream" });
  const emit = (value) =>
    response.write(`event: ${value.type}\ndata: ${JSON.stringify(value)}\n\n`);
  emit({ type: "response.created", response: { id: "response-fixture" } });
  emit({
    type: "response.output_item.added",
    output_index: 0,
    item: {
      type: "message",
      role: "assistant",
      id: "message-fixture",
      content: [],
    },
  });
  emit({
    type: "response.output_text.delta",
    item_id: "message-fixture",
    output_index: 0,
    content_index: 0,
    delta: text,
  });
  emit({
    type: "response.output_item.done",
    output_index: 0,
    item: {
      type: "message",
      role: "assistant",
      id: "message-fixture",
      content: [{ type: "output_text", text }],
    },
  });
  emit({
    type: "response.completed",
    response: {
      id: "response-fixture",
      usage: {
        input_tokens: 1,
        input_tokens_details: null,
        output_tokens: 2,
        output_tokens_details: null,
        total_tokens: 3,
      },
    },
  });
  response.end();
}
export async function nativeFixture(
  t,
  { controlled = false, handleModel, endpoint } = {},
) {
  const root = await realpath(
      await mkdtemp(join(tmpdir(), "rss-codex-native-")),
    ),
    requests = [],
    ports = [];
  const cwd = join(root, "cwd");
  await mkdir(cwd);
  const model = createServer(async (request, response) => {
    try {
      let body = "";
      for await (const chunk of request) body += chunk;
      requests.push(JSON.parse(body));
      if (handleModel)
        await handleModel(requests.at(-1), response, requests.length);
      else reply(response);
    } catch {
      response.writeHead(500).end();
    }
  });
  await new Promise((resolve) => model.listen(0, "127.0.0.1", resolve));
  const configuration = {
    namespace: fixtureSession().namespace,
    provider: "codex",
    config: { id: "native-fixture", revision: "1" },
    accountRef: "fixture-account",
    workingDirectory: cwd,
    permissions: controlled ? "host_mediated" : "tools_disabled",
    ...(controlled
      ? {
          tools: endpoint ?? {
            propose: async () => ({
              ok: true,
              value: { disposition: "rejected", text: "fixture denial" },
            }),
          },
          verifier: {
            verify: async () => ({
              ok: true,
              value: {
                platform: "darwin-arm64",
                verificationRef: "native-fixture",
              },
            }),
          },
        }
      : {}),
  };
  const lineage = new Map();
  const options = {
    resolveConfiguration: async (identity) => ({
      configuration: { ...configuration, namespace: identity.namespace },
      nativeDirectory: join(root, "native"),
      apiUrl: `http://127.0.0.1:${model.address().port}/v1`,
      apiKey: "fixture-not-a-real-key",
      model: "fixture-model",
      ...(identity.history && lineage.has(identity.history.nativeThreadId)
        ? { ownedHistory: lineage.get(identity.history.nativeThreadId) }
        : {}),
    }),
  };
  const make = () => {
    const port = createCodexAdapter(options);
    ports.push(port);
    return port;
  };
  t.after(async () => {
    for (const port of ports) unwrap(await port.close(budget()));
    model.closeAllConnections();
    await new Promise((resolve) => model.close(resolve));
    await rm(root, { recursive: true, force: true });
  });
  return { root, requests, configuration, options, make, lineage, ports };
}
export function prompt(binding, commandId) {
  return {
    command: {
      schemaVersion: 2,
      kind: "command",
      sessionId: "session-1",
      commandId,
      expiresAtMs: Date.now() + 60000,
      input: {
        type: "prompt",
        text: "Reply with fixture reply",
        policy: "queue_next",
      },
    },
    attempt: {
      attemptId: `attempt-${commandId}`,
      originGeneration: binding.generation,
      observerGeneration: binding.generation,
      nativeSessionId: binding.nativeSessionId,
      nativeThreadId: binding.nativeThreadId,
      certainty: "intent",
    },
  };
}
export async function conversation(port, admitted, id) {
  const { command, attempt } = prompt(admitted.binding, id);
  const submission = await port.submit(
    admitted.binding,
    command,
    attempt,
    budget(),
  );
  assert.equal(submission.certainty, "submitted");
  const observations = [];
  for await (const value of port.observe(admitted.binding, budget())) {
    observations.push(value);
    if (value.body?.type === "terminal") break;
  }
  assert.equal(observations.at(-1)?.body?.outcome, "completed");
  assert.equal(
    observations.find((v) => v.body?.type === "text")?.body.text,
    "fixture reply",
  );
  return { submission, observations, command, attempt };
}
