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
