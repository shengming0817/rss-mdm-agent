import { createServer } from "node:http";

/** Fixed model HTTP transport; no external model or credentials. */
export function createModelServer(replies, requests = []) {
  const server = createServer(async (req, res) => {
    try {
      let data = "";
      for await (const chunk of req) data += chunk;
      if (!req.url.startsWith("/v1/messages")) {
        res.writeHead(200, { "content-type": "application/json" });
        res.end("{}");
        return;
      }
      const body = JSON.parse(data);
      requests.push(body);
      const blocks = await replies.shift();
      if (!blocks) throw new Error("unexpected model request");
      if (blocks === "http-error") {
        res.writeHead(400, { "content-type": "application/json" });
        res.end(
          JSON.stringify({
            type: "error",
            error: {
              type: "invalid_request_error",
              message: "fixed fixture failure",
            },
          }),
        );
        return;
      }
      res.writeHead(200, { "content-type": "text/event-stream" });
      const emit = (type, info) =>
        res.write(
          `event: ${type}\ndata: ${JSON.stringify({ type, ...info })}\n\n`,
        );
      emit("message_start", {
        message: {
          id: `msg_fixture_${requests.length}`,
          type: "message",
          role: "assistant",
          model: "fixture-model",
          content: [],
          stop_reason: null,
          stop_sequence: null,
          usage: { input_tokens: 1, output_tokens: 0 },
        },
      });
      if (blocks === "hang") return;
      for (const [index, block] of blocks.entries()) {
        if (block.type === "text") {
          emit("content_block_start", {
            index,
            content_block: { type: "text", text: "" },
          });
          emit("content_block_delta", {
            index,
            delta: { type: "text_delta", text: block.text },
          });
        } else {
          emit("content_block_start", {
            index,
            content_block: { ...block, input: {} },
          });
          emit("content_block_delta", {
            index,
            delta: {
              type: "input_json_delta",
              partial_json: JSON.stringify(block.input),
            },
          });
        }
        emit("content_block_stop", { index });
      }
      emit("message_delta", {
        delta: {
          stop_reason: blocks.some((b) => b.type === "tool_use")
            ? "tool_use"
            : "end_turn",
          stop_sequence: null,
        },
        usage: { output_tokens: 1 },
      });
      emit("message_stop", {});
      res.end();
    } catch {
      res.writeHead(500);
      res.end();
    }
  });
  return server;
}
