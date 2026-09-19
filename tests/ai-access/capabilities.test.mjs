import assert from "node:assert/strict";
import { test } from "node:test";
import {
  FakeHost,
  fixtureCaller,
  unwrap,
} from "../../packages/ai-contract/dist/testing/index.js";
const budget = () => ({ signal: AbortSignal.timeout(1000), timeoutMs: 1000 });
test("scripted Host exercises queue, steer, cancel and continuation support and rejection", async () => {
  for (const supported of [true, false]) {
    const host = new FakeHost(
      undefined,
      { now: () => 0 },
      {
        steer: supported ? "supported" : "unsupported",
        cancellation: supported ? "request_only" : "unsupported",
        continuation: supported ? "same_process" : "unsupported",
      },
    );
    const s = unwrap(
      await host.createSession(
        fixtureCaller,
        {
          provider: "fake",
          config: { id: "c", revision: "1" },
          accountRef: "a",
          profile: "conversation",
        },
        budget(),
      ),
    );
    const command = {
      schemaVersion: 2,
      kind: "command",
      sessionId: s.namespace.sessionId,
      commandId: "first",
      expiresAtMs: 1000,
      input: { type: "prompt", policy: "queue_next", text: "a" },
    };
    unwrap(await host.submit(fixtureCaller, command, budget()));
    assert.equal(
      (await host.submit(fixtureCaller, command, budget())).ok,
      true,
    );
    assert.equal(
      (
        await host.submit(
          fixtureCaller,
          { ...command, commandId: "queued" },
          budget(),
        )
      ).ok,
      true,
    );
    unwrap(
      await host.advance(fixtureCaller, s.namespace.sessionId, "first", []),
    );
    const current = unwrap(await host.store.session(s.namespace));
    assert.equal(
      (
        await host.submit(
          fixtureCaller,
          {
            ...command,
            commandId: "steer",
            input: {
              type: "prompt",
              policy: "steer",
              targetRunId: current.binding.nativeRunId,
              text: "b",
            },
          },
          budget(),
        )
      ).ok,
      supported,
    );
    assert.equal(
      (
        await host.cancel(
          fixtureCaller,
          {
            ...command,
            commandId: "cancel",
            input: {
              type: "cancel",
              targetCommandId: "first",
              generation: current.binding.generation,
              nativeRunId: current.binding.nativeRunId,
            },
          },
          budget(),
        )
      ).ok,
      supported,
    );
    assert.equal(
      (await host.resume(fixtureCaller, s.namespace.sessionId, budget())).ok,
      supported,
    );
    await host.close(budget());
  }
});
