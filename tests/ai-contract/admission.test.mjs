import { activeStage } from "../../packages/ai-contract/dist/index.js";
import assert from "node:assert/strict";
import test from "node:test";
import { VerifiedProviderSession } from "../../packages/ai-contract/dist/session.js";
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
  namespace: fixtureSession().namespace,
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
        ...activeStage(fixtureSession()).binding,
        generation: "new",
        nativeSessionId: "wrong",
      },
      capabilities: {
        ...activeStage(fixtureSession()).capabilities,
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

test("restore rejects a different workspace before resuming the native session", async () => {
  const previous = fixtureSession();
  activeStage(previous).capabilities.continuation = "across_processes";
  let resumed = 0;
  const port = {
    resume: async () => {
      resumed++;
      return {
        ok: true,
        value: {
          binding: {
            ...activeStage(previous).binding,
            generation: "cross-directory",
          },
          capabilities: activeStage(previous).capabilities,
        },
      };
    },
    close: async () => ({ ok: true, value: { processStopped: true } }),
  };
  const result = await VerifiedProviderSession.restore(
    port,
    previous,
    { ...configuration, workingDirectory: "/different-workspace" },
    budget(),
  );
  assert.equal(result.ok, false);
  assert.equal(result.error.code, "permission_denied");
  assert.equal(resumed, 0);
});

test("fork admission rejects invalid child identity and retains cleanup ownership after an unknown native result", async () => {
  const parentPort = new ScriptedProvider();
  const create = parentPort.createSession.bind(parentPort);
  parentPort.createSession = async (...args) => {
    const value = unwrap(await create(...args));
    value.capabilities.fork = "supported";
    value.binding.nativeThreadId = "parent-thread";
    return { ok: true, value };
  };
  const parent = unwrap(
    await VerifiedProviderSession.open(parentPort, configuration, budget()),
  );
  for (const scenario of [
    "wrong-source",
    "same-native",
    "unknown",
    "cleanup-failed",
    "valid",
    "timeout",
  ]) {
    const agent = new ScriptedProvider();
    let closes = 0;
    agent.close = async () => {
      closes++;
      return {
        ok: true,
        value: { processStopped: scenario !== "cleanup-failed" },
      };
    };
    const child = {
      agent,
      diagnostics: { observe: async function* () {} },
      extensions: {
        fork: {
          forkSession: async (source, config, b) => {
            if (scenario === "timeout") return new Promise(() => {});
            if (["unknown", "cleanup-failed"].includes(scenario))
              return {
                certainty: "unknown",
                error: { code: "unavailable", retry: "reconcile_first" },
              };
            const value = unwrap(await agent.createSession(config, b));
            value.binding.nativeSessionId = "child-session";
            value.binding.nativeThreadId = "child-thread";
            value.binding.generation = "child-generation";
            if (scenario === "same-native")
              value.binding.nativeThreadId = parent.binding.nativeThreadId;
            if (scenario === "wrong-source")
              source.throughTurnId = "different-turn";
            return { certainty: "created", value, source };
          },
        },
      },
    };
    const result = await parent.fork(
      child,
      "terminal-turn",
      {
        ...configuration,
        namespace: { ...configuration.namespace, sessionId: "child" },
      },
      budget(),
    );
    if (scenario === "valid") {
      // A valid basic fork retains ownership until the Host explicitly closes it.
      assert.equal(result.certainty, "created");
      assert.equal(closes, 0);
      await child.agent.close(budget());
    } else {
      assert.equal(result.certainty, "unknown");
      assert.equal(closes, 1);
      assert.equal(Boolean(result.cleanupError), scenario === "cleanup-failed");
    }
  }
});

test("fork runs the child's controlled verifier and closes it after denial", async () => {
  const source = fixtureSession();
  activeStage(source).capabilities.fork = "supported";
  activeStage(source).capabilities.tools = "host_mediated";
  activeStage(source).binding.nativeThreadId = "parent-thread";
  let verifications = 0,
    closes = 0;
  const controlled = {
    ...configuration,
    permissions: "host_mediated",
  };
  const admission = {
    tools: { propose: async () => assert.fail("not a tool call") },
    verifier: {
      verify: async () =>
        ++verifications === 1
          ? {
              ok: true,
              value: { platform: "fixture", verificationRef: "fixture" },
            }
          : { ok: false, error: { code: "permission_denied", retry: "never" } },
    },
  };
  const parentPort = {
    createSession: async () => ({
      ok: true,
      value: {
        binding: activeStage(source).binding,
        capabilities: activeStage(source).capabilities,
      },
    }),
    close: async () => ({ ok: true, value: { processStopped: true } }),
  };
  const parent = unwrap(
    await VerifiedProviderSession.open(
      parentPort,
      controlled,
      budget(),
      admission,
    ),
  );
  const child = {
    agent: {
      close: async () => {
        closes++;
        return { ok: true, value: { processStopped: true } };
      },
    },
    diagnostics: {},
    extensions: {
      fork: {
        forkSession: async (sourceRequest) => ({
          certainty: "created",
          source: sourceRequest,
          value: {
            capabilities: activeStage(source).capabilities,
            binding: {
              ...activeStage(source).binding,
              generation: "child-generation",
              nativeSessionId: "child-native",
              nativeThreadId: "child-thread",
            },
          },
        }),
      },
    },
  };
  const result = await parent.fork(
    child,
    "terminal-turn",
    {
      ...controlled,
      namespace: { ...controlled.namespace, sessionId: "child" },
    },
    budget(),
    admission,
  );
  assert.equal(result.certainty, "unknown");
  assert.equal(result.error.code, "permission_denied");
  assert.equal(verifications, 2);
  assert.equal(closes, 1);
});
