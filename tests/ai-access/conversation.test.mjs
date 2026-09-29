import assert from "node:assert/strict";
import { test } from "node:test";
import { createAccessService } from "../../packages/ai-access/dist/index.js";
import {
  RuntimeClient,
  localTransportPair,
} from "../../packages/ai-client/dist/index.js";
import {
  FakeHost,
  fixtureCaller,
  unwrap,
} from "../../packages/ai-contract/dist/testing/index.js";

test("two clients receive the same full command without snapshot hydration and keep exact run identity", async () => {
  const host = new FakeHost(undefined, { now: () => 0 });
  const service = createAccessService({
    host,
    sessionOptions: { connectionId: "cfg" },
  });
  const clients = [];
  try {
    for (let i = 0; i < 2; i++) {
      const pair = localTransportPair();
      service.connect(pair[0], fixtureCaller);
      const client = new RuntimeClient(pair[1]);
      await client.initialize();
      clients.push(client);
    }
    const session = await clients[0].restore(
      (await clients[0].createSession({ sessionId: crypto.randomUUID() }))
        .sessionId,
    );
    const id = session.namespace.sessionId;
    await clients[1].restore(id);
    const original = host.snapshotPage.bind(host);
    let snapshots = 0;
    host.snapshotPage = (...args) => {
      snapshots++;
      return original(...args);
    };
    const command = {
      schemaVersion: 7,
      kind: "command",
      sessionId: id,
      commandId: "prompt-1",
      expiresAtMs: 1000,
      input: {
        type: "prompt",
        policy: "queue_next",
        text: "A real user question",
      },
    };
    await clients[0].submit(command);
    for (
      let i = 0;
      i < 100 && !clients[1].getSession(id)?.commands["prompt-1"];
      i++
    )
      await new Promise((r) => setTimeout(r, 5));
    assert.equal(clients[1].getSession(id).connection, "resync_required");
    await clients[1].restore(id);
    snapshots = 0;
    assert.deepEqual(
      clients[1].getSession(id).commands["prompt-1"].command,
      command,
    );
    assert.equal(snapshots, 0);
    unwrap(await host.advance(fixtureCaller, id, command.commandId, []));
    for (
      let i = 0;
      i < 100 &&
      !clients[1].getSession(id)?.commands["prompt-1"]?.dispatch?.nativeRunId;
      i++
    )
      await new Promise((r) => setTimeout(r, 5));
    assert.equal(
      clients[1].getSession(id).commands["prompt-1"].dispatch.nativeRunId,
      "run-prompt-1",
    );
    const restored = await clients[0].restore(id, 1);
    assert.deepEqual(
      restored.commands["prompt-1"],
      clients[1].getSession(id).commands["prompt-1"],
    );
  } finally {
    for (const c of clients) c.close();
    await service.close();
    await host.close({ timeoutMs: 1000, signal: new AbortController().signal });
  }
});

test("session/new preserves a stable caller identity and does not couple creation to attachment", async () => {
  const host = new FakeHost();
  const service = createAccessService({
    host,
    sessionOptions: { connectionId: "cfg" },
  });
  const pair = localTransportPair();
  service.connect(pair[0], fixtureCaller);
  const client = new RuntimeClient(pair[1]);
  try {
    await client.initialize();
    host.snapshotPage = async () => ({
      ok: false,
      error: { code: "unavailable", retry: "never" },
    });
    const intent = { sessionId: "lost-create-receipt", connectionId: "cfg" };
    assert.deepEqual(await client.createSession(intent), {
      sessionId: intent.sessionId,
    });
    assert.deepEqual(await client.createSession(intent), {
      sessionId: intent.sessionId,
    });
    assert.equal((await client.listSessions()).items.length, 1);
    await assert.rejects(client.restore(intent.sessionId));
    assert.deepEqual(await client.createSession(intent), {
      sessionId: intent.sessionId,
    });
  } finally {
    client.close();
    await service.close();
    await host.close({ timeoutMs: 1000, signal: new AbortController().signal });
  }
});
