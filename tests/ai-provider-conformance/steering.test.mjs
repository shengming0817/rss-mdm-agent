import assert from "node:assert/strict";
import test from "node:test";
import {
  engines,
  fixture,
  command,
  until,
  unwrap,
  budget,
  evidence,
} from "./support.mjs";

for (const provider of engines) {
  test(
    `${provider}: Host enforces the native steer capability`,
    { timeout: 90000 },
    async (t) => {
      const f = await fixture(t, provider);
      const release = f.model.hold();
      if (provider === "codex")
        f.model.text("steer consumed in the original turn");
      await f.start();
      const { client } = await f.connect();
      const view = await client.createSession(),
        id = view.namespace.sessionId;
      const second = (await f.connect()).client;
      await second.restore(id);
      await client.submit(command(id, "original"));
      await until(() => f.model.requests.length === 1, "held original request");
      const original =
        provider === "codex"
          ? await until(() => {
              const c = client.getSession(id)?.commands.original;
              return c?.state === "running" ? c : undefined;
            }, "native acceptance")
          : client.getSession(id).commands.original;
      const steer = {
        ...command(id, "steer"),
        input: {
          type: "prompt",
          policy: "steer",
          text: "Finish now",
          targetRunId:
            original.dispatch.nativeRunId ?? "unavailable-native-run",
        },
      };
      if (provider === "codex") {
        await second.submit(steer);
        await until(
          () => client.getSession(id)?.commands.steer?.state === "acknowledged",
          "native steer acknowledgement",
        );
        const redirected = client.getSession(id).commands.steer;
        assert.equal(redirected.acknowledgement.type, "steer");
        assert.equal(
          redirected.dispatch.nativeRunId,
          original.dispatch.nativeRunId,
        );
        assert.equal(redirected.outcome, undefined);
      } else {
        await assert.rejects(second.submit(steer), /unsupported_capability/);
        assert.equal(client.getSession(id).commands.steer, undefined);
      }
      assert.equal(
        f.model.requests.length,
        1,
        "steer does not dispatch another concurrent run",
      );
      release();
      await until(
        () => client.getSession(id)?.commands.original?.state === "terminal",
        "original native terminal",
      );
      const { session, events } = unwrap(
        await f.app.host.snapshotPage(
          f.config.caller,
          id,
          { limit: 256 },
          budget(),
        ),
      );
      assert.equal(
        client.getSession(id).commands.original.outcome,
        "completed",
      );
      assert.equal(events.filter((e) => e.body.type === "terminal").length, 1);
      assert.equal(f.model.requests.length, provider === "codex" ? 2 : 1);
      evidence(t, "native-steer-difference", session, f.model.requests, {
        result: provider === "codex" ? "supported" : "unsupported",
      });
    },
  );
}
