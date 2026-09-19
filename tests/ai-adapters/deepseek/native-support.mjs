import assert from "node:assert/strict";
import { createServer } from "node:http";
import { mkdtemp, rm, readdir, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DeepSeekAdapter } from "../../../packages/ai-adapters/deepseek/dist/adapter.js";
import { nativeRuntime } from "../../../packages/ai-adapters/deepseek/dist/runtime.js";
import { fingerprint } from "../../../packages/ai-contract/dist/index.js";
import { fixtureLimits } from "../../../packages/ai-contract/dist/testing/index.js";
import { VerifiedProviderSession } from "../../../packages/ai-contract/dist/session.js";
import {
  fixtureSession,
  fixtureCommand,
  fixtureAttempt,
  unwrap,
} from "../../../packages/ai-contract/dist/testing/index.js";
import { budget, configuration } from "./support.mjs";

export async function environment(t, handler, configure = () => {}) {
  const dir = await mkdtemp(join(tmpdir(), "rss-dsh-native-"));
  const requests = [];
  const server = createServer(async (req, res) => {
    let raw = "";
    for await (const chunk of req) raw += chunk;
    const body = JSON.parse(raw);
    requests.push(body);
    await handler(body, res, requests.length);
  });
  await new Promise((r) => server.listen(0, "127.0.0.1", r));
  const ports = [];
  t.after(async () => {
    for (const p of ports) await p.close(budget());
    server.closeAllConnections();
    await new Promise((r) => server.close(r));
    await rm(dir, { recursive: true, force: true });
  });
  const config = configuration(dir);
  configure(config);
  const admission = config.tools
    ? { tools: config.tools, verifier: config.verifier }
    : undefined;
  delete config.tools;
  delete config.verifier;
  function port(transformRuntime = (runtime) => runtime) {
    const result = new DeepSeekAdapter(
      {
        tools: admission?.tools,
        resolveConfiguration: async () => ({
          configuration: config,
          persistenceDirectory: dir,
          apiUrl: "https://custom.example.test/v1",
          apiKey: "fixture",
          model: "deepseek-chat",
        }),
      },
      () => {
        const runtime = nativeRuntime(),
          call = runtime.call.bind(runtime);
        runtime.call = (op, value, b) =>
          call(
            op,
            op === "initialize"
              ? {
                  ...value,
                  apiUrl: `http://127.0.0.1:${server.address().port}`,
                }
              : value,
            b,
          );
        return transformRuntime(runtime);
      },
    );
    ports.push(result);
    return result;
  }
  return { dir, config, admission, port, requests };
}
export function completion(res, text = "native answer") {
  res.writeHead(200, { "content-type": "text/event-stream" });
  for (const content of [text.slice(0, 6), text.slice(6)])
    res.write(
      `data: ${JSON.stringify({ id: "chat-fixture", object: "chat.completion.chunk", choices: [{ index: 0, delta: { content }, finish_reason: null }] })}\n\n`,
    );
  res.end(
    `data: ${JSON.stringify({ id: "chat-fixture", object: "chat.completion.chunk", choices: [{ index: 0, delta: {}, finish_reason: "stop" }] })}\n\ndata: [DONE]\n\n`,
  );
}
export function command(id = "command-1", text = "say hello") {
  return {
    ...fixtureCommand(id),
    expiresAtMs: Date.now() + 60000,
    input: { type: "prompt", policy: "queue_next", text },
  };
}
export async function collect(port, binding) {
  const events = [];
  for await (const event of port.observe(binding, budget(10000)))
    events.push(event);
  return events;
}
export async function files(dir) {
  const result = {};
  for (const e of await readdir(dir, { withFileTypes: true })) {
    const p = join(dir, e.name);
    if (e.isDirectory()) Object.assign(result, await files(p));
    else if (e.name.endsWith(".jsonl")) result[p] = await readFile(p, "utf8");
  }
  return result;
}
