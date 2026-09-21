import { activeStage } from "../../packages/ai-contract/dist/index.js";
import assert from "node:assert/strict";
import { test } from "node:test";
import {
  FakeHost,
  fixtureCaller,
  unwrap,
} from "../../packages/ai-contract/dist/testing/index.js";
const budget = () => ({ signal: AbortSignal.timeout(1000), timeoutMs: 1000 });
test("scripted Host exercises queue, steer, cancel and continuation support and rejection", async () => {
  for (const status of ["supported", "unsupported", "unknown"]) {
    const supported = status === "supported";
    const host = new FakeHost(
      undefined,
      { now: () => 0 },
      {
        steer: status,
        cancellation: supported ? "request_only" : status,
        continuation: supported ? "same_process" : status,
      },
    );
    const s = unwrap(
      await host.openSessionForTest(
        fixtureCaller,
        {
          provider: "fake",
          config: { id: "c", revision: "1" },

          profile: "conversation",
        },
        budget(),
      ),
    );
    const command = {
      schemaVersion: 5,
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
              targetRunId: activeStage(current).binding.nativeRunId,
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
              generation: activeStage(current).binding.generation,
              nativeRunId: activeStage(current).binding.nativeRunId,
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
