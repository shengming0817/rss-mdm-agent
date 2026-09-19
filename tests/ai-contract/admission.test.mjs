import assert from "node:assert/strict";
import test from "node:test";
import { VerifiedProviderSession } from "../../packages/ai-contract/dist/index.js";
import {
  ScriptedProvider,
  fixtureSession,
  unwrap,
} from "../../packages/ai-contract/dist/testing/index.js";
const configuration = {
  provider: "fake",
  config: { id: "config-1", revision: "1" },
  accountRef: "account-1",
  workingDirectory: ".",
  permissions: "tools_disabled",
};
const budget = () => ({ timeoutMs: 25, signal: new AbortController().signal });
test("post-create validation failure closes only the consumed provider instance", async () => {
  const port = new ScriptedProvider(),
    create = port.createSession.bind(port);
  let closes = 0;
  port.createSession = async (...args) => {
    const result = unwrap(await create(...args));
    result.binding.accountRef = "wrong";
    return { ok: true, value: result };
  };
  port.close = async (b) => {
    assert.equal(b.signal.aborted, false);
    closes++;
    return { ok: true, value: { processStopped: true } };
  };
  const failed = await VerifiedProviderSession.open(
    port,
    configuration,
    budget(),
  );
  assert.equal(failed.ok, false);
  assert.equal(failed.error.code, "permission_denied");
  assert.equal(closes, 1);
  assert.equal(
    (await VerifiedProviderSession.open(port, configuration, budget())).ok,
    false,
  );
  assert.equal(closes, 1);
});
test("resume identity rejection closes its new observer and retains the primary and cleanup failures", async () => {
  const port = new ScriptedProvider();
  let closes = 0;
  port.resume = async () => ({
    ok: true,
    value: {
      binding: {
        ...fixtureSession().binding,
        generation: "new",
        nativeSessionId: "wrong",
      },
      capabilities: {
        ...fixtureSession().capabilities,
        continuation: "across_processes",
      },
    },
  });
  port.close = async () => {
    closes++;
    return { ok: false, error: { code: "unavailable", retry: "same_command" } };
  };
  const result = await VerifiedProviderSession.restore(
    port,
    fixtureSession(),
    configuration,
    budget(),
  );
  assert.equal(result.ok, false);
  assert.equal(result.error.code, "permission_denied");
  assert.equal(result.cleanupError.code, "unavailable");
  assert.equal(closes, 1);
});
test("abort and a non-cooperative close return a bounded value-free cleanup result", async () => {
  const port = new ScriptedProvider();
  port.createSession = () => new Promise(() => {});
  port.close = () => new Promise(() => {});
  const start = performance.now(),
    result = await VerifiedProviderSession.open(port, configuration, budget());
  assert.equal(result.ok, false);
  assert.equal(result.cleanupError.code, "unavailable");
  assert.ok(performance.now() - start < 1000);
});
test("provider cannot emit host-owned lifecycle events through the observation port", async () => {
  const { runProviderConformance } = await import(
    "../../packages/ai-contract/dist/testing/index.js"
  );
  await assert.rejects(
    runProviderConformance(
      (scenario) => {
        const port = new ScriptedProvider();
        port.submission = scenario;
        port.observe = async function* (binding) {
          yield {
            type: "event",
            binding,
            attemptId: "attempt-command-1",
            commandId: "command-1",
            body: { type: "session_retired" },
          };
        };
        return port;
      },
      configuration,
      budget,
    ),
  );
});
