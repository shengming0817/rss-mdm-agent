import { activeStage } from "../../packages/ai-contract/dist/index.js";
import assert from "node:assert/strict";
import test from "node:test";
import {
  engines,
  fixture,
  command,
  until,
  unwrap,
  budget,
  capabilities,
  evidence,
} from "./support.mjs";

for (const provider of engines) {
  test(
    `${provider}: native Host sessions, two-client ledger, FIFO and disconnected replay`,
    { timeout: 90000 },
    async (t) => {
      const f = await fixture(t, provider);
      const release = f.model.hold();
      f.model.text("second answer");
      await f.start();
      const first = await f.connect(),
        second = await f.connect();
      const view = await first.client.createSession(),
        id = view.namespace.sessionId;
      let { session } = unwrap(
        await f.app.host.snapshotPage(
          f.config.caller,
          id,
          { limit: 256 },
          budget(),
        ),
      );
      assert.equal(session.currentStageId, undefined);
      assert.deepEqual(session.stages, []);
      for (const field of ["tenantId", "principalId", "authorityId"]) {
        const caller = { ...f.config.caller, [field]: "foreign" };
        const probe = command(id, "foreign-" + field);
        const cancel = {
          ...probe,
          input: {
            type: "cancel",
            generation: "foreign-generation",
            targetCommandId: "first",
          },
        };
        const answer = {
          ...probe,
          input: {
            type: "respond",
            generation: "foreign-generation",
            interactionId: "question",
            answer: {},
          },
        };
        for (const result of [
          await f.app.host.snapshotPage(caller, id, { limit: 256 }, budget()),
          await f.app.host.submit(caller, probe, budget()),
          await f.app.host.cancel(caller, cancel, budget()),
          await f.app.host.respond(caller, answer, budget()),
        ])
          assert.equal(
            result.ok,
            false,
            "foreign caller must not access the session",
          );
      }
      for (const patch of [
        { accountRef: "foreign" },
        { config: { id: "local", revision: "r2" } },
        { profile: "controlled_tools" },
        { provider: "foreign" },
      ])
        assert.equal(
          (
            await f.app.host.createSession(
              f.config.caller,
              { ...f.config.session, ...patch },
              budget(),
            )
          ).ok,
          false,
        );
      await second.client.restore(id);
      const input = command(id, "first");
      const receipts = await Promise.all([
        first.client.submit(input),
        second.client.submit(input),
      ]);
      assert.deepEqual(receipts[0], receipts[1]);
      session = unwrap(
        await f.app.host.snapshotPage(
          f.config.caller,
          id,
          { limit: 256 },
          budget(),
        ),
      ).session;
      assert.deepEqual(
        activeStage(session).capabilities,
        capabilities(provider),
      );
      assert.equal(activeStage(session).binding.provider, provider);
      assert.deepEqual(
        activeStage(session).binding.config,
        f.config.session.config,
      );
      assert.equal(
        activeStage(session).binding.accountRef,
        f.config.session.accountRef,
      );

      await assert.rejects(
        second.client.submit({
          ...input,
          input: { ...input.input, text: "different" },
        }),
        /content_conflict/,
      );
      await until(() => f.model.requests.length === 1, "first native request");
      const queued = command(id, "next", "second turn");
      await second.client.submit(queued);
      const current = unwrap(
        await f.app.host.snapshotPage(
          f.config.caller,
          id,
          { limit: 256 },
          budget(),
        ),
      );
      assert.equal(
        current.commands.find((r) => r.command.commandId === "next").state,
        "accepted",
      );
      assert.equal(
        f.model.requests.length,
        1,
        "Host queue must not dispatch a second active turn",
      );
      first.close();
      const reconnected = await f.connect();
      const restored = await reconnected.client.restore(id);
      assert.notEqual(restored.commands.first.state, "terminal");
      release();
      await until(
        () =>
          reconnected.client.getSession(id)?.commands.next?.state ===
          "terminal",
        "queued terminal",
      ).catch(async (error) => {
        t.diagnostic(
          JSON.stringify(
            unwrap(
              await f.app.host.snapshotPage(
                f.config.caller,
                id,
                { limit: 256 },
                budget(),
              ),
            ).commands,
          ),
        );
        throw error;
      });
      const before = reconnected.client.getSession(id);
      assert.equal(before.commands.first.outcome, "completed");
      assert.equal(before.commands.next.outcome, "completed");
      assert.equal(f.model.requests.length, 2);
      const after = await reconnected.client.restore(id, 1);
      assert.deepEqual(after.messages, before.messages);
      assert.deepEqual(after.timeline, before.timeline);
      assert.equal(after.cursor, before.cursor);
      // More turns than the worker's bounded observation slots exposes leaked streams.
      for (let turn = 3; turn <= 6; turn++) {
        f.model.text("answer-" + turn);
        const name = "turn-" + turn;
        await reconnected.client.submit(command(id, name));
        await until(
          () =>
            reconnected.client.getSession(id)?.commands[name]?.state ===
            "terminal",
          name,
        );
        assert.equal(
          reconnected.client.getSession(id).commands[name].outcome,
          "completed",
        );
      }
      assert.equal(f.model.requests.length, 6);
      evidence(
        t,
        "native-basic-ledger-queue-replay",
        session,
        f.model.requests,
        {
          result: "supported",
        },
      );
    },
  );
}
