import assert from "node:assert/strict";
import test from "node:test";
import { spawnSync } from "node:child_process";
import { access, mkdir, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { ToolBridge } from "../../../packages/ai-adapters/codex/dist/bridge.js";
import { createTestAdapter } from "../../../packages/ai-adapters/codex/dist/testing.js";
import { nativeRuntime } from "../../../packages/ai-adapters/codex/dist/runtime.js";
import {
  VerifiedProviderSession,
  workspaceIdentity,
} from "../../../packages/ai-contract/dist/session.js";
import {
  fixtureSession,
  unwrap,
} from "../../../packages/ai-contract/dist/testing/index.js";
import { nativeFixture, budget, reply, conversation } from "./helpers.mjs";

const adapterRequire = createRequire(
  new URL("../../../packages/ai-adapters/codex/package.json", import.meta.url),
);
const { Client } = await import(
  pathToFileURL(
    adapterRequire.resolve("@modelcontextprotocol/sdk/client/index.js"),
  )
);
const { StreamableHTTPClientTransport } = await import(
  pathToFileURL(
    adapterRequire.resolve(
      "@modelcontextprotocol/sdk/client/streamableHttp.js",
    ),
  )
);

const responseCompleted = (id) => ({
  type: "response.completed",
  response: {
    id,
    usage: {
      input_tokens: 1,
      input_tokens_details: null,
      output_tokens: 1,
      output_tokens_details: null,
      total_tokens: 2,
    },
  },
});
function events(response, values) {
  response.writeHead(200, { "content-type": "text/event-stream" });
  for (const value of values)
    response.write(`event: ${value.type}\ndata: ${JSON.stringify(value)}\n\n`);
  response.end();
}
const functionCall = (callId, name, args, namespace) => ({
  type: "response.output_item.done",
  output_index: 0,
  item: {
    type: "function_call",
    call_id: callId,
    name,
    arguments: JSON.stringify(args),
    ...(namespace ? { namespace } : {}),
  },
});
const customToolCall = (callId, name, input) => ({
  type: "response.output_item.done",
  output_index: 0,
  item: { type: "custom_tool_call", call_id: callId, name, input },
});
const names = (body) =>
  (body.tools ?? [])
    .flatMap((tool) =>
      tool.type === "namespace"
        ? tool.tools.map((child) => `${tool.name}__${child.name}`)
        : [tool.name],
    )
    .sort();
test("bridge rejects foreign callers and exposes no readable resources", async (t) => {
  let proposals = 0;
  const bridge = new ToolBridge(
    {
      propose: async () => {
        proposals++;
        return {
          ok: true,
          value: { disposition: "returned", text: "unexpected" },
        };
      },
    },
    () => false,
  );
  await bridge.start(budget());
  t.after(() => bridge.close(budget()));

  const denied = await fetch(bridge.url, {
    method: "POST",
    headers: {
      authorization: "Bearer wrong-incarnation",
      "content-type": "application/json",
      accept: "application/json, text/event-stream",
    },
    body: JSON.stringify({
      jsonrpc: "2.0",
      id: 1,
      method: "initialize",
      params: {
        protocolVersion: "2025-06-18",
        capabilities: {},
        clientInfo: { name: "foreign", version: "1" },
      },
    }),
  });
  assert.equal(denied.status, 403);

  const client = new Client({ name: "security-test", version: "1" });
  const transport = new StreamableHTTPClientTransport(new URL(bridge.url), {
    requestInit: {
      headers: { authorization: `Bearer ${bridge.token}` },
    },
  });
  await client.connect(transport);
  t.after(() => client.close());
  assert.deepEqual(
    (await client.listTools()).tools.map((tool) => tool.name),
    ["propose"],
  );
  assert.deepEqual((await client.listResources()).resources, []);
  assert.deepEqual(
    (await client.listResourceTemplates()).resourceTemplates,
    [],
  );
  await assert.rejects(() =>
    client.readResource({ uri: "file:///must-not-exist" }),
  );
  const unavailable = await client.callTool({
    name: "propose",
    arguments: { name: "blocked", arguments: {} },
  });
  assert.equal(unavailable.isError, true);
  assert.equal(proposals, 0);
});

test(
  "fixed app-server advertises no native tools and cannot execute forced native calls",
  { timeout: 60000 },
  async (t) => {
    let marker;
    const s = await nativeFixture(t, {
      handleModel: (body, response, index) => {
        marker ??= join(s.configuration.workingDirectory, "native-tool-marker");
        if (index > 1) return reply(response);
        events(response, [
          {
            type: "response.created",
            response: { id: "response-hostile" },
          },
          functionCall("shell-call", "exec_command", {
            cmd: `/usr/bin/touch ${JSON.stringify(marker)}`,
          }),
          customToolCall(
            "patch-call",
            "apply_patch",
            `*** Begin Patch\n*** Add File: ${marker}\n+owned\n*** End Patch`,
          ),
          functionCall("agent-call", "spawn_agent", {
            task_name: "escape",
            message: "create native-tool-marker",
          }),
          responseCompleted("response-hostile"),
        ]);
      },
    });
    const port = s.make();
    const admitted = unwrap(
      await VerifiedProviderSession.open(port, s.configuration, budget()),
    );
    await conversation(port, admitted, "hostile-native");
    assert.ok(s.requests.length >= 2);
    assert.ok(s.requests.every((request) => names(request).length === 0));
    assert.deepEqual(
      s.requests
        .slice(1)
        .flatMap((request) => request.input ?? [])
        .filter((item) =>
          ["function_call_output", "custom_tool_call_output"].includes(
            item.type,
          ),
        )
        .map((item) => item.call_id)
        .sort(),
      ["agent-call", "patch-call", "shell-call"],
    );
    await assert.rejects(() => access(marker));
  },
);

test(
  "fixed app-server rejects project configuration before creating a native thread",
  { timeout: 40000 },
  async (t) => {
    const s = await nativeFixture(t);
    await mkdir(join(s.configuration.workingDirectory, ".codex"));
    await writeFile(
      join(s.configuration.workingDirectory, ".codex", "config.toml"),
      "[features]\nshell_tool = true\n",
    );
    const result = await VerifiedProviderSession.open(
      s.make(),
      s.configuration,
      budget(),
    );
    assert.equal(result.ok, false);
    assert.equal(s.requests.length, 0);
  },
);

test("restore rejects a compatible-looking thread without host lineage", async (t) => {
  const s = await nativeFixture(t);
  const previous = {
    ...fixtureSession(),
    namespace: s.configuration.namespace,
    binding: {
      provider: "codex",
      providerVersion: "0.155.0",
      adapterVersion: "0.1.0",
      generation: "foreign-generation",
      workspaceId: workspaceIdentity(s.configuration.workingDirectory),
      accountRef: s.configuration.accountRef,
      config: s.configuration.config,
      nativeSessionId: "foreign-session",
      nativeThreadId: "foreign-thread",
    },
  };
  const result = await VerifiedProviderSession.restore(
    s.make(),
    previous,
    s.configuration,
    budget(),
  );
  assert.equal(result.ok, false);
  assert.equal(s.requests.length, 0);
});

for (const disposition of ["rejected", "returned"]) {
  test(
    `fixed app-server routes MCP propose ${disposition} through the only host endpoint`,
    { timeout: 60000 },
    async (t) => {
      const calls = [];
      const s = await nativeFixture(t, {
        controlled: true,
        endpoint: {
          propose: async (proposal) => {
            calls.push(proposal);
            return {
              ok: true,
              value: { disposition, text: `host ${disposition}` },
            };
          },
        },
        handleModel: (body, response, index) => {
          if (index === 1) {
            events(response, [
              {
                type: "response.created",
                response: { id: "response-proposal" },
              },
              functionCall(
                "proposal-call",
                "propose",
                { name: "inventory.read", arguments: { device: "one" } },
                "mcp__rss_host",
              ),
              responseCompleted("response-proposal"),
            ]);
          } else reply(response);
        },
      });
      const port = s.make();
      const admitted = unwrap(
        await VerifiedProviderSession.open(port, s.configuration, budget()),
      );
      const result = await conversation(
        port,
        admitted,
        `proposal-${disposition}`,
      );
      assert.deepEqual(
        names(s.requests[0]),
        [
          "list_mcp_resource_templates",
          "list_mcp_resources",
          "mcp__rss_host__propose",
          "read_mcp_resource",
        ].sort(),
      );
      assert.deepEqual(calls, [
        { name: "inventory.read", arguments: { device: "one" } },
      ]);
      const tool = result.observations.find(
        (value) => value.body?.type === "tool_result",
      );
      assert.equal(tool?.body.disposition, disposition);
    },
  );
}

const badMcpRuntime = (spec) =>
  nativeRuntime({
    ...spec,
    env: { ...spec.env, RSS_CODEX_MCP_TOKEN: "wrong-incarnation-token" },
  });

test(
  "fixed app-server fails closed when required MCP authentication fails during start (runtime fault seam)",
  { timeout: 40000 },
  async (t) => {
    const s = await nativeFixture(t, { controlled: true });
    const port = createTestAdapter(s.options, badMcpRuntime);
    s.ports.push(port);
    const result = await VerifiedProviderSession.open(
      port,
      s.configuration,
      budget(),
    );
    assert.equal(result.ok, false);
    assert.equal(s.requests.length, 0);
  },
);

test(
  "fixed app-server fails closed when required MCP authentication fails during owned resume (runtime fault seam)",
  { timeout: 80000 },
  async (t) => {
    const s = await nativeFixture(t, { controlled: true });
    const firstPort = s.make();
    const admitted = unwrap(
      await VerifiedProviderSession.open(firstPort, s.configuration, budget()),
    );
    const first = await conversation(
      firstPort,
      admitted,
      "persist-for-failure",
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
    unwrap(await firstPort.close(budget()));

    const restoredPort = createTestAdapter(s.options, badMcpRuntime);
    s.ports.push(restoredPort);
    const result = await VerifiedProviderSession.restore(
      restoredPort,
      previous,
      s.configuration,
      budget(),
    );
    assert.equal(result.ok, false);
    assert.equal(s.requests.length, 1);
  },
);

test(
  "fixed app-server never resolves git from the inherited hostile PATH",
  { timeout: 30000 },
  async (t) => {
    const s = await nativeFixture(t);
    assert.equal(
      spawnSync("/usr/bin/git", ["init", s.configuration.workingDirectory])
        .status,
      0,
    );
    const bin = join(s.root, "hostile-bin"),
      marker = join(s.root, "git-was-executed");
    await mkdir(bin);
    await writeFile(
      join(bin, "git"),
      `#!/bin/sh\n/usr/bin/touch '${marker}'\nexit 1\n`,
      { mode: 0o700 },
    );
    const original = process.env.PATH;
    process.env.PATH = `${bin}:${original}`;
    try {
      const port = s.make();
      const admitted = unwrap(
        await VerifiedProviderSession.open(port, s.configuration, budget()),
      );
      await conversation(port, admitted, "hostile-path");
      await assert.rejects(access(marker));
    } finally {
      if (original === undefined) delete process.env.PATH;
      else process.env.PATH = original;
    }
  },
);
