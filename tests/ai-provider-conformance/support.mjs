import assert from "node:assert/strict";
import { createServer } from "node:http";
import { connect } from "node:net";
import { once } from "node:events";
import { Readable, Writable } from "node:stream";
import { mkdtemp, mkdir, writeFile, rm, realpath } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { startLocalApp } from "../../apps/ai-host/dist/index.js";
import {
  RuntimeClient,
  ndJsonStream,
} from "../../packages/ai-client/dist/index.js";
import { createModelServer } from "../ai-adapters/claude/model-fixture.mjs";
import { reply } from "../ai-adapters/codex/helpers.mjs";
import { completion } from "../ai-adapters/deepseek/native-support.mjs";
import {
  profileDigest,
  nativeToolInventory,
} from "../../scripts/check-ai-acceptance.mjs";
export { capabilities } from "../../scripts/check-ai-acceptance.mjs";

export const engines = ["codex", "claude", "deepseek"];
export const budget = (timeoutMs = 15000) => ({
  timeoutMs,
  signal: AbortSignal.timeout(timeoutMs),
});
export const unwrap = (result) => {
  assert.equal(result.ok, true, JSON.stringify(result));
  return result.value;
};
export async function until(check, label = "condition", timeoutMs = 15000) {
  const end = Date.now() + timeoutMs;
  while (Date.now() < end) {
    const result = await check();
    if (result) return result;
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
  assert.fail(`Timed out: ${label}`);
}
export function command(sessionId, commandId, text = "hello") {
  return {
    schemaVersion: 4,
    kind: "command",
    sessionId,
    commandId,
    expiresAtMs: Date.now() + 120000,
    input: { type: "prompt", policy: "queue_next", text },
  };
}
/** A real native process consumes this local protocol server. No real model evidence. */
export async function modelServer(provider) {
  const replies = [],
    requests = [],
    releases = [];
  const server =
    provider === "claude"
      ? createModelServer(replies, requests)
      : createServer(async (req, res) => {
          try {
            let raw = "";
            for await (const chunk of req) raw += chunk;
            requests.push(JSON.parse(raw));
            const text = await replies.shift();
            if (typeof text === "function") return text(res);
            if (typeof text !== "string")
              throw Error("unexpected model request");
            if (provider === "codex") reply(res, text);
            else completion(res, text);
          } catch {
            res.writeHead(500).end();
          }
        });
  const value = (text) =>
    provider === "claude" ? [{ type: "text", text }] : text;
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  return {
    requests,
    replies,
    apiUrl: `http://127.0.0.1:${server.address().port}${provider === "codex" ? "/v1" : ""}`,
    text(text = "native answer") {
      replies.push(value(text));
    },
    question() {
      if (provider === "claude")
        replies.push([
          {
            type: "tool_use",
            id: "question-" + replies.length,
            name: "AskUserQuestion",
            input: {
              questions: [
                {
                  question: "Choose?",
                  header: "Choice",
                  options: [
                    { label: "A", description: "a" },
                    { label: "B", description: "b" },
                  ],
                  multiSelect: false,
                },
              ],
            },
          },
        ]);
      else if (provider === "deepseek")
        replies.push((res) => {
          res.writeHead(200, { "content-type": "text/event-stream" });
          const chunk = (delta, finish_reason = null) =>
            res.write(
              `data: ${JSON.stringify({
                id: "question",
                object: "chat.completion.chunk",
                choices: [{ index: 0, delta, finish_reason }],
              })}\n\n`,
            );
          chunk({
            tool_calls: [
              {
                index: 0,
                id: "question",
                type: "function",
                function: {
                  name: "ask_user_question",
                  arguments: JSON.stringify({
                    questions: [
                      {
                        id: "q",
                        question: "Choose?",
                        options: [{ label: "yes" }, { label: "no" }],
                      },
                    ],
                  }),
                },
              },
            ],
          });
          chunk({}, "tool_calls");
          res.end("data: [DONE]\n\n");
        });
      else throw Error("Codex does not advertise structured questions");
    },
    hold() {
      let resolve;
      replies.push(
        new Promise((done) => {
          resolve = done;
        }),
      );
      const release = (text = "native answer") => resolve(value(text));
      releases.push(release);
      return release;
    },
    async close() {
      for (const release of releases) release();
      server.closeAllConnections();
      await new Promise((resolve) => server.close(resolve));
    },
  };
}
export async function configuration(
  directory,
  provider,
  apiUrl,
  profile = "conversation",
) {
  const nativeDirectory = join(directory, "native");
  await mkdir(nativeDirectory, { mode: 0o700 });
  const credentialPath = join(directory, "credential");
  await writeFile(credentialPath, "fixture-only-key", { mode: 0o600 });
  const config = {
    databasePath: join(directory, "ai.sqlite"),
    socketPath: join(directory, "ai.sock"),
    caller: {
      tenantId: "s1-test",
      principalId: "fixture-actor",
      authorityId: "desktop-fixture",
    },
    session: {
      provider,
      config: { id: "local", revision: "r1" },
      accountRef: "test-account",
      profile,
    },
    workingDirectory: directory,
    nativeDirectory,
    connection: {
      source: "custom_endpoint",
      credentialPath,
      credentialType: "api_key",
      apiUrl,
      model: provider === "deepseek" ? "deepseek-chat" : "fixture-model",
    },
  };
  const path = join(directory, "configuration.json");
  await writeFile(path, JSON.stringify(config), { mode: 0o600 });
  return { config, path };
}
export async function clientAt(socketPath) {
  const socket = connect(socketPath);
  await once(socket, "connect");
  const client = new RuntimeClient(
    ndJsonStream(Writable.toWeb(socket), Readable.toWeb(socket)),
  );
  await client.initialize();
  return {
    client,
    close() {
      client.close();
      socket.destroy();
    },
  };
}
export async function fixture(t, provider) {
  const directory = await realpath(await mkdtemp(join(tmpdir(), "rss-a06-")));
  const model = await modelServer(provider);
  const { config, path } = await configuration(
    directory,
    provider,
    model.apiUrl,
  );
  let app;
  const peers = [];
  const f = {
    directory,
    model,
    config,
    path,
    get app() {
      return app;
    },
    async start() {
      app = await startLocalApp(path);
      return app;
    },
    async stop() {
      for (const peer of peers.splice(0)) peer.close();
      await app?.close();
      app = undefined;
    },
    async connect() {
      const peer = await clientAt(config.socketPath);
      peers.push(peer);
      return peer;
    },
  };
  t.after(async () => {
    try {
      await f.stop();
    } finally {
      await model.close();
    }
    await rm(directory, { recursive: true, force: true });
  });
  return f;
}
export function evidence(t, scenario, session, requests, extra = {}) {
  const { workspaceId: _workspace, ...binding } = session.binding;
  t.diagnostic(
    JSON.stringify({
      a06: 1,
      profileSourceSha256: profileDigest(binding.provider),
      scenario,
      proof: "real_process_local_model",
      provider: binding.provider,
      profile:
        session.capabilities.tools === "disabled"
          ? "conversation"
          : "controlled_tools",
      binding,
      capabilities: session.capabilities,
      ...extra,
      modelRequests: requests.length,
      nativeTools: nativeToolInventory(requests),
    }),
  );
}
