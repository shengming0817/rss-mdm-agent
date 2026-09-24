import { CODEX_VERSION } from "../../packages/ai-adapters/codex/dist/runtime.js";
import { PROVIDER_VERSION } from "../../packages/ai-adapters/claude/dist/configuration.js";
import { HARNESS_VERSION } from "../../packages/ai-adapters/deepseek/dist/configuration.js";
import { COMPOSITION_ID } from "../../packages/ai-adapters/deepseek/dist/assembly.js";
import { activeStage } from "../../packages/ai-contract/dist/index.js";
import { openSqliteStore } from "../../packages/ai-store-sqlite/dist/index.js";
import { ConnectionSecrets } from "../../apps/ai-host/dist/secrets.js";
import { NativeControl } from "../../apps/ai-host/dist/native.js";
import { PrivateLink } from "../../packages/ai-host/dist/private-link.js";
import { spawn } from "node:child_process";
import { executionServer } from "../ai-host/rust-execution.mjs";
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { once } from "node:events";
import { PassThrough } from "node:stream";
import {
  mkdtemp,
  mkdir,
  readFile,
  writeFile,
  rm,
  realpath,
} from "node:fs/promises";
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
    schemaVersion: 5,
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
  const config = {
    version: 1,
    databasePath: join(directory, "ai.sqlite"),
    nativeDirectory,
    workingDirectory: directory,
    caller: {
      tenantId: "test-users",
      principalId: "fixture-actor",
      authorityId: "desktop-fixture",
    },
    session: {
      provider,
      config: { id: "local", revision: "1" },

      profile,
    },
    connection: {
      schemaVersion: 5,
      kind: "connection",
      connectionId: "local",
      name: "Native fixture",
      provider,
      configRevision: 1,

      profile,
      status: "ready",
      source: {
        type: "custom_api",
        apiUrl,
        credentialType: "api_key",
        model: provider === "deepseek" ? "deepseek-chat" : "fixture-model",
      },
    },
  };
  const store = unwrap(
    openSqliteStore({ path: config.databasePath, mode: "create" }),
  );
  const secrets = new ConnectionSecrets(store, async () => Buffer.alloc(32, 7));
  unwrap(
    await store.saveConnection(
      config.caller,
      config.connection,
      null,
      await secrets.seal(config.caller, config.connection, "fixture-only-key"),
    ),
  );
  await store.close(budget());
  const path = join(directory, "configuration.json");
  await writeConfiguration(path, config);
  return { config, path };
}
export async function writeConfiguration(path, config) {
  const { caller, session, connection, ...paths } = config;
  await writeFile(path, JSON.stringify(paths), { mode: 0o600 });
}
export function nativePeer(socket) {
  const inputs = new Map();
  const control = new NativeControl(
    async (call) => {
      if (call.method !== "masterKey") throw Error("unexpected parent request");
      return [...Buffer.alloc(32, 7)];
    },
    ({ channel, message }) => inputs.get(channel)?.enqueue(message),
    socket,
  );
  return { control, inputs, next: 0 };
}
export async function executionGeneration(directory) {
  const path = join(directory, "execution-user.json");
  const deadline = Date.now() + 10000;
  while (Date.now() < deadline) {
    try {
      const record = JSON.parse(await readFile(path, "utf8"));
      assert.equal(typeof record.generation, "string");
      return record.generation;
    } catch (error) {
      if (error.code !== "ENOENT") throw error;
      await new Promise((resolve) => setTimeout(resolve, 20));
    }
  }
  assert.fail("execution user generation unavailable");
}
export async function clientAt(parent, generation = "fixture-generation") {
  const channel = `fixture-view-${++parent.next}`;
  const readable = new ReadableStream({
    start(input) {
      parent.inputs.set(channel, input);
    },
  });
  const writable = new WritableStream({
    write(message) {
      parent.control.emit(channel, message);
    },
  });
  await parent.control.call("attach", {
    channel,
    context: {
      schemaVersion: 5,
      kind: "userContext",
      generation,
      user: {
        schemaVersion: 5,
        kind: "testUser",
        userId: "fixture-actor",
        displayName: "Fixture",
        nameKey: "fixture",
      },
    },
  });
  const client = new RuntimeClient({ readable, writable });
  await client.initialize();
  return {
    client,
    close() {
      client.close();
      parent.inputs.delete(channel);
      void parent.control.call("detach", { channel }).catch(() => {});
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
  let app, rust, parent, link;
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
      rust = spawn(
        executionServer(),
        [
          join(directory, "execution.sqlite"),
          join(directory, "audit.json"),
          "ai-unknown",
        ],
        { stdio: ["pipe", "pipe", "ignore"] },
      );
      rust.stdin.on("error", () => {});
      const toHost = new PassThrough(),
        fromHost = new PassThrough();
      link = new PrivateLink(fromHost, toHost, "native");
      link.lane("execution").pipe(rust.stdin);
      rust.stdout.pipe(link.lane("execution"));
      parent = nativePeer(link.lane("native"));
      app = await startLocalApp(path, { input: toHost, output: fromHost });
      return app;
    },
    async stop() {
      for (const peer of peers.splice(0)) peer.close();
      await app?.close();
      app = undefined;
      parent?.control.close();
      parent = undefined;
      link?.close();
      link = undefined;
      if (rust && rust.exitCode === null && rust.signalCode === null) {
        const exited = once(rust, "exit");
        rust.kill("SIGTERM");
        await exited;
      }
      rust = undefined;
    },
    async connect() {
      const peer = await clientAt(parent, await executionGeneration(directory));
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
export function capabilities(provider, tools = "disabled") {
  return {
    continuation: "across_processes",
    cancellation: "request_only",
    tools,
    steer: provider === "codex" ? "supported" : "unsupported",
    fork: provider === "codex" ? "supported" : "unsupported",
    subagent: "unsupported",
    terminal: "unsupported",
    structuredQuestion: provider === "codex" ? "unsupported" : "supported",
    multimodal: "unsupported",
  };
}
const id = (value) =>
  typeof value === "string" && value.length > 0 && value.length <= 512;
/** Observed request definitions only; never derive this inventory from capability declarations. */
export function nativeToolInventory(requests) {
  if (!Array.isArray(requests) || requests.length === 0)
    throw new Error("No native model request");
  const names = requests.flatMap((request) => {
    const tools = request.tools ?? [];
    if (!Array.isArray(tools)) throw new Error("Invalid native tool inventory");
    return tools.flatMap((tool) => {
      if (tool?.type === "namespace") {
        if (!id(tool.name) || !Array.isArray(tool.tools))
          throw new Error("Invalid native tool inventory");
        return tool.tools.map((child) => {
          if (!id(child?.name))
            throw new Error("Invalid native tool inventory");
          return `${tool.name}__${child.name}`;
        });
      }
      const name = tool?.name ?? tool?.function?.name;
      if (!id(name)) throw new Error("Invalid native tool inventory");
      return name;
    });
  });
  return [...new Set(names)].sort();
}
const providerVersions = {
  codex: CODEX_VERSION,
  claude: PROVIDER_VERSION,
  deepseek: `harness-${HARNESS_VERSION}.${COMPOSITION_ID}`,
};
const adapterVersions = Object.fromEntries(
  await Promise.all(
    engines.map(async (provider) => [
      provider,
      JSON.parse(
        await readFile(
          new URL(
            `../../packages/ai-adapters/${provider}/package.json`,
            import.meta.url,
          ),
          "utf8",
        ),
      ).version,
    ]),
  ),
);
export function assertNativeSession(
  provider,
  session,
  requests,
  tools = "disabled",
) {
  const stage = activeStage(session),
    binding = stage.binding;
  assert.equal(binding.provider, provider);
  assert.equal(binding.providerVersion, providerVersions[binding.provider]);
  assert.equal(binding.adapterVersion, adapterVersions[binding.provider]);
  assert.ok(id(binding.generation) && id(binding.nativeSessionId));
  if (binding.provider === "codex") assert.ok(id(binding.nativeThreadId));
  for (const key of ["nativeRunId", "nativeRequestId"])
    assert.ok(binding[key] === undefined || id(binding[key]));
  assert.deepEqual(binding.config, {
    id: "local",
    revision: tools === "disabled" ? "1" : "2",
  });
  assert.deepEqual(stage.capabilities, capabilities(binding.provider, tools));
  assert.ok(requests.length > 0);
  assert.deepEqual(
    nativeToolInventory(requests),
    binding.provider === "codex"
      ? tools === "disabled"
        ? []
        : [
            "list_mcp_resource_templates",
            "list_mcp_resources",
            "mcp__rss_host__propose",
            "read_mcp_resource",
          ]
      : [
          binding.provider === "claude"
            ? "AskUserQuestion"
            : "ask_user_question",
        ],
  );
}
