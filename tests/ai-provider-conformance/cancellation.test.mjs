import assert from "node:assert/strict";
import test from "node:test";
import {
  engines,
  fixture,
  command,
  until,
  unwrap,
  budget,
  assertNativeSession,
} from "./support.mjs";

for (const provider of engines) {
  test(
    `${provider}: cancellation acknowledgement is distinct from native terminal`,
    { timeout: 90000 },
    async (t) => {
      const f = await fixture(t, provider);
      let release;
      if (provider === "deepseek") f.model.question();
      else if (provider === "claude") f.model.replies.push("hang");
      else release = f.model.hold();
      await f.start();
      const { client } = await f.connect();
      const view = await client.createSession(),
        id = view.namespace.sessionId;
      await client.submit(command(id, "long"));
      await until(
        () =>
          client.getSession(id)?.commands.long?.dispatch?.certainty ===
          "submitted",
        "native submission",
      );
      await until(() => f.model.requests.length === 1, "native model request");
      if (provider === "deepseek")
        await until(
          () =>
            Object.values(client.getSession(id).interactions).some(
              (q) => q.status === "pending",
            ),
          "native callback",
        );
      const nativeRunId =
        client.getSession(id).commands.long.dispatch.nativeRunId;
      const cancel = {
        ...command(id, "cancel"),
        input: {
          type: "cancel",
          generation: client.getSession(id).generation,
          targetCommandId: "long",
          ...(nativeRunId ? { nativeRunId } : {}),
        },
      };
      await assert.rejects(
        client.submit({
          ...cancel,
          commandId: "old-cancel",
          input: { ...cancel.input, generation: "old" },
        }),
        /stale_binding/,
      );
      const receipt = await client.submit(cancel);
      assert.deepEqual(await client.submit(cancel), receipt);
      await until(
        () => client.getSession(id)?.commands.cancel?.state === "acknowledged",
        "control acknowledgement",
      );
      const acknowledged = client.getSession(id);
      assert.equal(acknowledged.commands.cancel.acknowledgement.type, "cancel");
      assert.equal(
        acknowledged.commands.cancel.acknowledgement.confirmation,
        "request_only",
      );
      assert.equal(acknowledged.commands.cancel.outcome, undefined);
      release?.();
      await until(
        () => client.getSession(id)?.commands.long?.state === "terminal",
        "native cancellation terminal",
      );
      assert.equal(client.getSession(id).commands.long.outcome, "cancelled");
      assert.equal(f.model.requests.length, 1);
      const { session, events } = unwrap(
        await f.app.host.snapshotPage(
          f.config.caller,
          id,
          { limit: 256 },
          budget(),
        ),
      );
      assert.equal(
        events.filter(
          (e) => e.commandId === "long" && e.body.type === "terminal",
        ).length,
        1,
      );
      assertNativeSession(session, f.model.requests);
    },
  );
}

test(
  "deepseek: cancellation with no native terminal remains unknown across Host restart",
  { timeout: 90000 },
  async (t) => {
    const f = await fixture(t, "deepseek");
    f.model.hold();
    await f.start();
    let { client } = await f.connect();
    const view = await client.createSession(),
      id = view.namespace.sessionId;
    const prompt = command(id, "held");
    const original = await client.submit(prompt);
    await until(() => f.model.requests.length === 1, "held model request");
    await client.submit({
      ...command(id, "cancel-held"),
      input: {
        type: "cancel",
        generation: client.getSession(id).generation,
        targetCommandId: "held",
      },
    });
    await until(
      () =>
        client.getSession(id)?.commands["cancel-held"]?.state ===
        "acknowledged",
      "cancel acknowledged",
    );
    assert.equal(client.getSession(id).commands.held.outcome, undefined);
    await f.stop();
    await f.start();
    ({ client } = await f.connect());
    const restored = await client.restore(id, 1);
    assert.equal(restored.commands.held.state, "reconciliation_required");
    assert.equal(restored.commands.held.outcome, undefined);
    assert.deepEqual(await client.submit(prompt), original);
    assert.equal(f.model.requests.length, 1);
    const { session } = unwrap(
      await f.app.host.snapshotPage(
        f.config.caller,
        id,
        { limit: 256 },
        budget(),
      ),
    );
    assertNativeSession(session, f.model.requests);
  },
);
