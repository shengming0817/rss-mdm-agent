import assert from "node:assert/strict";
import test from "node:test";
import {
  fixture,
  command,
  until,
  unwrap,
  budget,
  evidence,
} from "./support.mjs";

for (const provider of ["claude", "deepseek"]) {
  test(
    `${provider}: two clients answer once; restart keeps a lost question unavailable`,
    { timeout: 90000 },
    async (t) => {
      const f = await fixture(t, provider);
      f.model.question();
      f.model.text("answered");
      f.model.question();
      await f.start();
      const a = await f.connect(),
        b = await f.connect();
      const view = await a.client.createSession(),
        id = view.namespace.sessionId;
      await b.client.restore(id);
      await a.client.submit(command(id, "question-1"));
      const pending = (client) =>
        Object.entries(client.getSession(id)?.interactions ?? {})
          .map(([interactionId, q]) => ({ ...q, interactionId }))
          .find((q) => q.status === "pending");
      const first = await until(() => pending(a.client), "native question");
      const answer = (question, commandId) => ({
        ...command(id, commandId),
        input: {
          type: "respond",
          interactionId: question.interactionId,
          generation: question.generation,
          ...(question.nativeRunId
            ? { nativeRunId: question.nativeRunId }
            : {}),
          answer:
            provider === "claude"
              ? { answers: { "Choose?": "A" } }
              : { answers: [{ id: "q", selected: ["yes"] }] },
        },
      });
      const old = answer(first, "stale");
      old.input.generation = "old-generation";
      await assert.rejects(b.client.submit(old), /stale_binding/);
      const contenders = [answer(first, "answer-a"), answer(first, "answer-b")];
      const replies = await Promise.allSettled([
        a.client.submit(contenders[0]),
        b.client.submit(contenders[1]),
      ]);
      assert.equal(replies.filter((r) => r.status === "fulfilled").length, 1);
      assert.match(
        replies.find((r) => r.status === "rejected").reason.message,
        /already_answered|revision_conflict/,
      );
      const winner = replies.findIndex((r) => r.status === "fulfilled");
      assert.deepEqual(
        await a.client.submit(contenders[winner]),
        replies[winner].value,
      );
      await until(
        () =>
          a.client.getSession(id)?.commands["question-1"]?.state === "terminal",
        "answered terminal",
      );
      assert.equal(f.model.requests.length, 2);
      assert.match(
        JSON.stringify(f.model.requests[1]),
        provider === "claude" ? /Choose\?/ : /yes/,
      );
      await assert.rejects(
        a.client.submit(answer(first, "late")),
        /already_answered/,
      );

      await a.client.submit(command(id, "question-2"));
      const lost = await until(() => {
        const q = pending(a.client);
        return q?.interactionId !== first.interactionId ? q : undefined;
      }, "next native question");
      const before = a.client.getSession(id);
      await f.stop();
      await f.start();
      const resumed = await f.connect();
      const restored = await resumed.client.restore(id, 1);
      assert.deepEqual(restored.messages, before.messages);
      assert.equal(
        restored.interactions[lost.interactionId].status,
        "unavailable",
      );
      await assert.rejects(
        resumed.client.submit(answer(lost, "after-restart")),
        /reconciliation_required/,
      );
      assert.equal(
        f.model.requests.length,
        3,
        "display recovery must not recreate the callback",
      );
      const { session } = unwrap(
        await f.app.host.snapshotPage(
          f.config.caller,
          id,
          { limit: 256 },
          budget(),
        ),
      );
      evidence(
        t,
        "question-answer-race-lost-callback",
        session,
        f.model.requests,
        {
          result: "supported",
        },
      );
    },
  );
}
