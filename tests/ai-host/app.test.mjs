import assert from "node:assert/strict";
import { test } from "node:test";
import { connect } from "node:net";
import { once } from "node:events";
import { Readable, Writable } from "node:stream";
import { mkdtemp, mkdir, writeFile, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { startLocalApp } from "../../apps/ai-host/dist/index.js";
import {
  RuntimeClient,
  ndJsonStream,
} from "../../packages/ai-client/dist/index.js";
import { createModelServer } from "../ai-adapters/claude/model-fixture.mjs";

test("local app uses a private ACP socket and a real SDK worker against fixed model transport", async () => {
  let finishReply;
  const heldReply = new Promise((resolve) => {
    finishReply = () => resolve([{ type: "text", text: "real isolated SDK" }]);
  });
  const directory = await mkdtemp(join(tmpdir(), "rss-host-app-")),
    requests = [],
    server = createModelServer([heldReply], requests);
  let app, client, socket;
  try {
    await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
    const configDirectory = join(directory, "claude");
    await mkdir(configDirectory, { mode: 0o700 });
    const credentialPath = join(directory, "credential");
    await writeFile(credentialPath, "fixture-only-key", { mode: 0o600 });
    const configuration = {
      databasePath: join(directory, "host.sqlite"),
      socketPath: join(directory, "host.sock"),
      caller: { tenantId: "t", principalId: "p", authorityId: "a" },
      session: {
        provider: "claude",
        config: { id: "native", revision: "1" },
        accountRef: "fixture",
        profile: "conversation",
      },
      workingDirectory: directory,
      claude: {
        configurationDirectory: configDirectory,
        credentialPath,
        credentialType: "api_key",
        apiUrl: `http://127.0.0.1:${server.address().port}`,
        model: "fixture-model",
      },
    };
    const path = join(directory, "configuration.json");
    await writeFile(path, JSON.stringify(configuration), { mode: 0o600 });
    app = await startLocalApp(path);
    socket = connect(configuration.socketPath);
    await once(socket, "connect");
    client = new RuntimeClient(
      ndJsonStream(Writable.toWeb(socket), Readable.toWeb(socket)),
    );
    await client.initialize();
    const session = await client.createSession(),
      id = session.namespace.sessionId;
    await client.submit({
      schemaVersion: 2,
      kind: "command",
      commandId: "native",
      sessionId: id,
      expiresAtMs: Date.now() + 30000,
      input: { type: "prompt", policy: "queue_next", text: "hello" },
    });
    const requestDeadline = Date.now() + 15000;
    while (requests.length === 0 && Date.now() < requestDeadline)
      await new Promise((resolve) => setTimeout(resolve, 20));
    assert.equal(requests.length, 1);
    const disconnected = once(socket, "close");
    socket.destroy();
    await disconnected;
    await client.close();
    socket = connect(configuration.socketPath);
    await once(socket, "connect");
    client = new RuntimeClient(
      ndJsonStream(Writable.toWeb(socket), Readable.toWeb(socket)),
    );
    await client.initialize();
    const restored = await client.restore(id);
    assert.equal(restored.namespace.sessionId, id);
    assert.equal(restored.status, "active");
    assert.notEqual(restored.commands.native.state, "terminal");
    finishReply();
    const deadline = Date.now() + 15000;
    while (
      Date.now() < deadline &&
      client.getSession(id).commands.native?.state !== "terminal"
    )
      await new Promise((resolve) => setTimeout(resolve, 20));
    assert.equal(client.getSession(id).commands.native?.outcome, "completed");
    assert.ok(
      Object.values(client.getSession(id).messages).some(
        (message) => message.text === "real isolated SDK",
      ),
    );
    await client.close();
    socket.destroy();
    client = undefined;
    const page = await app.host.listSessions(
      configuration.caller,
      { limit: 64 },
      { timeoutMs: 1000, signal: new AbortController().signal },
    );
    assert.equal(page.ok, true);
    assert.equal(page.value.items.length, 1);
    assert.equal(requests.length, 1);
  } finally {
    finishReply();
    await client?.close();
    socket?.destroy();
    await app?.close();
    server.closeAllConnections();
    await new Promise((resolve) => server.close(resolve));
    await rm(directory, { recursive: true, force: true });
  }
});
