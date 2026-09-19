import assert from "node:assert/strict";
import { test } from "node:test";
import {
  FakeHost,
  runHostConformance,
  fixtureCaller,
  fixtureCommand,
  unwrap,
} from "../../packages/ai-contract/dist/testing/index.js";
const options = {
  provider: "fake",
  config: { id: "cfg", revision: "1" },
  accountRef: "account-1",
  profile: "conversation",
};
const budget = () => ({ timeoutMs: 1000, signal: AbortSignal.timeout(1000) });
test("shared Host conformance runs against fake acceptance and attach", () =>
  runHostConformance(() => new FakeHost()));
test("controlled tools fail closed, unsupported steer and stale cancel do not dispatch", async () => {
  const host = new FakeHost();
  assert.equal(
    (
      await host.createSession(
        fixtureCaller,
        { ...options, profile: "controlled_tools" },
        budget(),
      )
    ).ok,
    false,
  );
  const session = unwrap(
    await host.createSession(fixtureCaller, options, budget()),
  );
  const command = {
    ...fixtureCommand(),
    sessionId: session.namespace.sessionId,
  };
  unwrap(await host.submit(fixtureCaller, command, budget()));
  assert.equal(
    (
      await host.submit(
        fixtureCaller,
        {
          ...command,
          input: {
            type: "prompt",
            text: "change",
            policy: "steer",
            targetRunId: "run-1",
          },
        },
        budget(),
      )
    ).ok,
    false,
  );
  assert.equal(
    (
      await host.cancel(
        fixtureCaller,
        {
          ...command,
          commandId: "cancel-1",
          input: {
            type: "cancel",
            targetCommandId: command.commandId,
            generation: "stale",
          },
        },
        budget(),
      )
    ).error.code,
    "stale_binding",
  );
  const receipt = unwrap(
    await host.cancel(
      fixtureCaller,
      {
        ...command,
        commandId: "cancel-1",
        input: {
          type: "cancel",
          targetCommandId: command.commandId,
          generation: session.binding.generation,
        },
      },
      budget(),
    ),
  );
  assert.equal(receipt.commandId, "cancel-1");
  assert.equal(
    unwrap(await host.snapshot(fixtureCaller, command.sessionId, budget()))
      .commands[0].state,
    "accepted",
  );
});

test("interleaved delta messages retain identities through Host subscription", async () => {
  const host = new FakeHost();
  const session = unwrap(
    await host.createSession(fixtureCaller, options, budget()),
  );
  const command = {
    ...fixtureCommand(),
    sessionId: session.namespace.sessionId,
  };
  unwrap(await host.submit(fixtureCaller, command, budget()));
  const control = new AbortController();
  const stream = host
    .subscribe(fixtureCaller, command.sessionId, 1, {
      timeoutMs: 1000,
      signal: control.signal,
    })
    [Symbol.asyncIterator]();
  try {
    let pending = stream.next();
    const messages = new Map();
    for (const [messageId, text] of [
      ["m1", "hel"],
      ["m2", "wor"],
      ["m1", "lo"],
      ["m2", "ld"],
    ]) {
      unwrap(
        await host.publishDelta(fixtureCaller, command.sessionId, {
          type: "delta",
          binding: session.binding,
          commandId: command.commandId,
          messageId,
          text,
        }),
      );
      const { value } = await pending;
      assert.equal(value.type, "delta");
      assert.equal(value.messageId, messageId);
      messages.set(messageId, (messages.get(messageId) ?? "") + value.text);
      pending = stream.next();
    }
    assert.deepEqual(
      [...messages],
      [
        ["m1", "hello"],
        ["m2", "world"],
      ],
    );
    unwrap(await host.close(budget()));
    assert.equal((await pending).done, true);
  } finally {
    control.abort();
    await stream.return();
    await host.close(budget());
  }
});

test("Host conformance preserves both body and close errors", async () => {
  const primary = new Error("body failed"),
    cleanup = new Error("close failed");
  const host = new FakeHost();
  host.negotiate = () => {
    throw primary;
  };
  host.close = async () => {
    throw cleanup;
  };
  await assert.rejects(
    runHostConformance(() => host),
    (error) =>
      error instanceof AggregateError &&
      error.errors[0] === primary &&
      error.errors[1].errors[0] === cleanup,
  );
});
