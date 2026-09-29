// Responses protocol fixture for the pinned, real Codex app-server. No fake Host or IPC.
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { randomUUID } from "node:crypto";

function stream(response, items) {
  const id = `response-${randomUUID()}`;
  response.writeHead(200, { "content-type": "text/event-stream" });
  for (const value of [
    { type: "response.created", response: { id } },
    ...items,
    {
      type: "response.completed",
      response: {
        id,
        usage: { input_tokens: 1, output_tokens: 1, total_tokens: 2 },
      },
    },
  ])
    response.write(`event: ${value.type}\ndata: ${JSON.stringify(value)}\n\n`);
  response.end();
}
function reply(response, text) {
  const item = {
    type: "message",
    role: "assistant",
    id: `msg-${randomUUID()}`,
    content: [{ type: "output_text", text }],
  };
  stream(response, [
    {
      type: "response.output_item.added",
      output_index: 0,
      item: { ...item, content: [] },
    },
    {
      type: "response.output_text.delta",
      item_id: item.id,
      output_index: 0,
      content_index: 0,
      delta: text,
    },
    { type: "response.output_item.done", output_index: 0, item },
  ]);
}
function propose(response, callId, name, args) {
  stream(response, [
    {
      type: "response.output_item.done",
      output_index: 0,
      item: {
        type: "function_call",
        call_id: callId,
        name: "propose",
        namespace: "mcp__rss_host",
        arguments: JSON.stringify({ name, arguments: args }),
      },
    },
  ]);
}
// Codex adds timing metadata and can truncate the middle of a large catalog.
// The reference precedes items; consume only that intact Rust-issued object.
function catalogFrom(value) {
  if (typeof value === "string") {
    try {
      const reference = /^\{"result":\{"catalog":(\{.*?\}),"items":/s.exec(
        value,
      );
      if (reference) return JSON.parse(reference[1]);
      return catalogFrom(
        JSON.parse(value.replace(/^Wall time: [\d.]+ seconds\nOutput:\n/, "")),
      );
    } catch {
      return;
    }
  }
  if (!value || typeof value !== "object") return;
  if (value.status === "ok" && value.result?.catalog)
    return value.result.catalog;
  for (const child of Object.values(value)) {
    const found = catalogFrom(child);
    if (found) return found;
  }
}
export async function startModelFixture() {
  const secret = `synthetic-native-${randomUUID()}`;
  let authorized = false,
    failure;
  const facts = {
    requests: 0,
    rejectedAuthentication: 0,
    proposals: [],
    completions: [],
  };
  const server = createServer(async (request, response) => {
    try {
      assert.equal(request.url, "/v1/responses");
      assert.equal(request.headers.authorization, `Bearer ${secret}`);
      const chunks = [];
      let size = 0;
      for await (const chunk of request) {
        size += chunk.length;
        assert.ok(size <= 4 * 1024 * 1024);
        chunks.push(chunk);
      }
      const body = JSON.parse(Buffer.concat(chunks));
      assert.equal(body.model, "native-golden-model");
      facts.requests++;
      if (!authorized) {
        facts.rejectedAuthentication++;
        response.writeHead(401);
        response.end(
          '{"error":{"message":"fixture authentication rejected","type":"invalid_request_error","code":"invalid_api_key"}}',
        );
        return;
      }
      const input = body.input ?? [];
      const user = input.filter((item) => item.role === "user").at(-1);
      const text =
        typeof user?.content === "string"
          ? user.content
          : (user?.content ?? []).map((x) => x.text ?? "").join("\n");
      const scenario = text.includes("connection_probe")
        ? "probe"
        : /GOLDEN_(INSTALL|DENY|CANCEL_REJECT|CANCEL_ALLOW|READ|HELLO)/.exec(
            text,
          )?.[1];
      assert.ok(scenario, "unknown fixture prompt");
      const output = (step) =>
        input.find(
          (item) =>
            item.type === "function_call_output" &&
            item.call_id === `${scenario}-${step}`,
        );
      const call = (step, name, args) => {
        facts.proposals.push(name);
        propose(response, `${scenario}-${step}`, name, args);
      };
      if (scenario === "probe" && !output("probe"))
        return call("probe", "connection_probe", {});
      if (["INSTALL", "DENY"].includes(scenario)) {
        if (!output("catalog")) return call("catalog", "execution_catalog", {});
        const catalog = catalogFrom(output("catalog").output);
        assert.ok(catalog, "Rust catalog unavailable");
        if (!output("execute"))
          return call("execute", "execution_execute", {
            catalog: {
              selection: {
                operationRequestId:
                  scenario === "INSTALL"
                    ? "native-golden-install"
                    : "native-golden-denied",
                catalog,
                itemId: "office",
                variantId: "test",
                arguments: { edition: "standard" },
              },
            },
          });
      }
      if (scenario.startsWith("CANCEL") && !output("cancel"))
        return call("cancel", "execution_cancel", {
          operationRequestId: "native-golden-install",
        });
      if (scenario === "READ") {
        if (!output("capabilities"))
          return call("capabilities", "execution_capabilities", {});
        if (!output("status"))
          return call("status", "execution_status", {
            operationRequestId: "native-golden-install",
          });
      }
      facts.completions.push(scenario);
      reply(
        response,
        scenario === "probe"
          ? "OK"
          : `完成 ${scenario}\n${"这是本地模型响应夹具，用于检查真实 WebView 的对话布局与持久历史。\n".repeat(12)}`,
      );
    } catch (error) {
      failure = error;
      response.writeHead(500);
      response.end("fixture failed");
    }
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  return {
    secret,
    facts,
    get failure() {
      return failure;
    },
    url: `http://127.0.0.1:${server.address().port}/v1`,
    accept() {
      authorized = true;
    },
    close: () =>
      new Promise((resolve) => {
        server.close(resolve);
        server.closeAllConnections();
      }),
  };
}
