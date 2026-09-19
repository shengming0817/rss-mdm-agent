import assert from "node:assert/strict";
import { client } from "@agentclientprotocol/sdk";
import { createAccessService, ndJsonStream } from "@rss-mdm-agent/ai-access";
import {
  FakeHost,
  fixtureCaller,
  unwrap,
} from "@rss-mdm-agent/ai-contract/testing";
const host = new FakeHost(),
  service = createAccessService({
    host,
    now: () => 0,
    sessionOptions: {
      provider: "fake",
      config: { id: "cfg", revision: "1" },
      accountRef: "a",
      profile: "conversation",
    },
  });
const a = new TransformStream(),
  b = new TransformStream();
const server = service.connect(
  ndJsonStream(a.writable, b.readable),
  fixtureCaller,
);
const updates = [];
const connection = client()
  .onNotification("session/update", ({ params }) => updates.push(params))
  .connect(ndJsonStream(b.writable, a.readable));
try {
  await connection.agent.request("initialize", {
    protocolVersion: 1,
    clientCapabilities: {},
  });
  const { sessionId } = await connection.agent.request("session/new", {
    cwd: "/",
    mcpServers: [],
  });
  const pending = connection.agent.request("session/prompt", {
    sessionId,
    prompt: [{ type: "text", text: "packed standard consumer" }],
  });
  let command;
  for (let i = 0; i < 200 && !command; i++) {
    command = unwrap(
      await host.store.snapshotPage(
        { ...fixtureCaller, sessionId },
        { limit: 64 },
      ),
    ).commands[0]?.command;
    if (!command) await new Promise((r) => setTimeout(r, 5));
  }
  assert.ok(command);
  unwrap(
    await host.advance(fixtureCaller, sessionId, command.commandId, [
      { type: "text", messageId: "answer", text: "Packed ACP works" },
      { type: "terminal", outcome: "completed" },
    ]),
  );
  assert.equal((await pending).stopReason, "end_turn");
  assert.equal(updates[0].update.content.text, "Packed ACP works");
  console.log(
    "PASS packed standard ACP via official ndJsonStream without Tauri",
  );
} finally {
  connection.close();
  server.close();
  await service.close();
}
