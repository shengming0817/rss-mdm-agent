import assert from "node:assert/strict";
import { test } from "node:test";
import {
  ScriptedProvider,
  runProviderConformance,
  fixtureCommand,
  fixtureLimits,
  unwrap,
} from "../../packages/ai-contract/dist/testing/index.js";
const configuration = {
  provider: "fake",
  config: { id: "config-1", revision: "1" },
  accountRef: "account-1",
  workingDirectory: ".",
  permissions: "tools_disabled",
};
const budget = () => ({ timeoutMs: 1000, signal: AbortSignal.timeout(1000) });
const callback = {
  category: "question",
  interactionId: "question-1",
  nativeCallbackId: "callback-1",
  expiresAtMs: 100,
  callbackLifetime: "generation_bound",
  request: { question: "Choose" },
};
const callbackHarness = (mutate) =>
  runProviderConformance(
    (scenario) => {
      const port = new ScriptedProvider();
      port.submission = scenario;
      port.observe = async function* (binding) {
        if (scenario === "submitted")
          yield mutate({
            type: "interaction",
            attemptId: "attempt-command-1",
            binding,
            commandId: fixtureCommand().commandId,
            interaction: structuredClone(callback),
          });
      };
      return port;
    },
    configuration,
    budget,
  );
test("provider conformance accepts a distinct live callback", () =>
  callbackHarness((x) => x));
for (const [name, mutate] of [
  [
    "permission callback",
    (x) => {
      x.interaction.category = "tool_permission";
      return x;
    },
  ],
  [
    "unclassified callback",
    (x) => {
      delete x.interaction.category;
      return x;
    },
  ],
  [
    "pending callback via generic event",
    (x) => ({
      type: "event",
      binding: x.binding,
      commandId: x.commandId,
      attemptId: x.attemptId,
      body: {
        type: "interaction",
        interactionId: x.interaction.interactionId,
        status: "pending",
        request: x.interaction.request,
      },
    }),
  ],
  [
    "legacy callback",
    (x) => {
      delete x.interaction.nativeCallbackId;
      x.interaction.nativeRequestId = "old";
      return x;
    },
  ],
  [
    "missing request",
    (x) => {
      delete x.interaction.request;
      return x;
    },
  ],
  [
    "oversized request",
    (x) => {
      x.interaction.request = {
        question: "x".repeat(fixtureLimits.maxTextBytes + 1),
      };
      return x;
    },
  ],
  ...[
    "accountRef",
    "nativeSessionId",
    "nativeRequestId",
    "provider",
    "nativeRunId",
  ].map((key) => [
    key,
    (x) => ({ ...x, binding: { ...x.binding, [key]: "wrong" } }),
  ]),
])
  test(`provider conformance rejects ${name}`, () =>
    assert.rejects(callbackHarness(mutate)));
test("shared provider lifecycle conformance runs without Tauri", () =>
  runProviderConformance(
    (scenario) => {
      const port = new ScriptedProvider();
      port.submission = scenario;
      return port;
    },
    configuration,
    budget,
  ));
test("unknown submit and cancel request do not manufacture terminal or tool authority", async () => {
  const port = new ScriptedProvider();
  const { binding } = unwrap(await port.createSession(configuration, budget()));
  port.submission = "unknown";
  assert.equal(
    (await port.submit(binding, fixtureCommand(), intent(binding), budget()))
      .certainty,
    "unknown",
  );
  assert.equal(
    (await port.cancel(binding, fixtureCommand(), budget())).value,
    "request_only",
  );
  assert.equal(
    (await Array.fromAsync(port.observe(binding, budget()))).length,
    0,
  );
  assert.equal(
    (
      await port.createSession(
        {
          ...configuration,
          permissions: "host_mediated",
          tools: {
            propose: () => {
              throw new Error("must never execute");
            },
          },
        },
        budget(),
      )
    ).ok,
    false,
  );
});

test("shared provider harness closes failed adapters with a fresh bounded signal", async () => {
  const primary = new Error("scripted initialization failed");
  let closed = false;
  const port = new ScriptedProvider();
  port.createSession = async () => {
    throw primary;
  };
  port.close = async (cleanup) => {
    closed = true;
    assert.equal(cleanup.signal.aborted, false);
    assert.ok(cleanup.timeoutMs > 0);
    return { ok: true, value: { processStopped: true } };
  };
  const abort = new AbortController();
  abort.abort();
  await assert.rejects(
    runProviderConformance(() => port, configuration, budget),
    primary,
  );
  assert.equal(closed, true);
});

test("equivalent binding property order does not reject submission", async () => {
  const port = new ScriptedProvider();
  const { binding } = unwrap(await port.createSession(configuration, budget()));
  const reordered = Object.fromEntries(Object.entries(binding).reverse());
  assert.equal(
    (
      await port.submit(
        reordered,
        fixtureCommand(),
        intent(reordered),
        budget(),
      )
    ).certainty,
    "submitted",
  );
});

test("conformance watchdog reaches cleanup when an adapter ignores abort", async () => {
  const port = new ScriptedProvider();
  let closed = false;
  port.createSession = () => new Promise(() => {});
  port.close = async () => {
    closed = true;
    return { ok: true, value: { processStopped: true } };
  };
  let timer;
  try {
    await assert.rejects(
      Promise.race([
        runProviderConformance(
          () => port,
          configuration,
          () => ({ timeoutMs: 20, signal: new AbortController().signal }),
        ),
        new Promise((_, reject) => {
          timer = setTimeout(
            () => reject(new Error("outer test guard expired")),
            200,
          );
        }),
      ]),
      /budget/,
    );
    assert.equal(closed, true);
  } finally {
    clearTimeout(timer);
  }
});

import { VerifiedProviderSession } from "../../packages/ai-contract/dist/index.js";
import { fixtureSession } from "../../packages/ai-contract/dist/testing/index.js";

test("controlled admission requires a trusted verifier and binds immutable evidence to endpoint and incarnation", async () => {
  let calls = 0,
    verified = 0;
  const tools = {
    propose: async () => ({
      ok: true,
      value: { disposition: "rejected", text: "fixture" },
    }),
  };
  const facts = {
    binding: fixtureSession().binding,
    capabilities: { ...fixtureSession().capabilities, tools: "host_mediated" },
  };
  const port = new ScriptedProvider();
  port.createSession = async () => {
    calls++;
    return { ok: true, value: facts };
  };
  const verifier = {
    verify: async (session, endpoint) => {
      verified++;
      assert.deepEqual(session, facts);
      assert.equal(endpoint, tools);
      // Mutation of the verifier's copy must not change what the admission signs.
      session.binding.generation = "tampered";
      return {
        ok: true,
        value: { platform: "fixture-only", verificationRef: "fixture-proof" },
      };
    },
  };
  for (const patch of [
    { permissions: "host_mediated" },
    { permissions: "host_mediated", tools },
    { permissions: "tools_disabled", tools },
    { permissions: "host_mediated", tools: {}, verifier },
  ])
    assert.equal(
      (
        await VerifiedProviderSession.open(
          port,
          { ...configuration, ...patch },
          budget(),
        )
      ).ok,
      false,
    );
  assert.equal(calls, 0);
  const config = {
    ...configuration,
    permissions: "host_mediated",
    tools,
    verifier,
  };
  const admitted = unwrap(
    await VerifiedProviderSession.open(port, config, budget()),
  );
  assert.equal(verified, 1);
  assert.equal(admitted.matches(facts.binding, tools), true);
  assert.equal(admitted.matches(facts.binding, { ...tools }), false);
  for (const patch of [
    { provider: "other" },
    { providerVersion: "other" },
    { adapterVersion: "other" },
    { generation: "other" },
    { accountRef: "other" },
    { nativeSessionId: "other" },
    { config: { id: "config-1", revision: "other" } },
  ])
    assert.equal(
      admitted.matches({ ...facts.binding, ...patch }, tools),
      false,
    );
  const copy = admitted.binding;
  copy.generation = "forged";
  assert.equal(admitted.matches(copy, tools), false);
  assert.throws(
    () =>
      new VerifiedProviderSession(
        Symbol("forged"),
        facts.binding,
        facts.capabilities,
      ),
    /unverified/,
  );
  assert.throws(
    () =>
      VerifiedProviderSession.prototype.matches.call(
        { ...admitted },
        facts.binding,
        tools,
      ),
    TypeError,
  );
  assert.equal(
    (
      await VerifiedProviderSession.open(
        port,
        { ...config, accountRef: "other" },
        budget(),
      )
    ).ok,
    false,
  );
  assert.equal(
    (
      await VerifiedProviderSession.open(
        port,
        { ...config, config: { id: "different", revision: "1" } },
        budget(),
      )
    ).ok,
    false,
  );
  assert.equal(
    (
      await VerifiedProviderSession.open(
        port,
        {
          ...config,
          verifier: {
            verify: async () => ({
              ok: false,
              error: { code: "permission_denied", retry: "never" },
            }),
          },
        },
        budget(),
      )
    ).ok,
    false,
  );
});

test("atomic provider session results retain their own capabilities across concurrent reinitialization", async () => {
  const port = new ScriptedProvider();
  const [first, second] = await Promise.all([
    VerifiedProviderSession.open(port, configuration, budget()).then(unwrap),
    VerifiedProviderSession.open(
      port,
      { ...configuration, config: { id: "config-2", revision: "2" } },
      budget(),
    ).then(unwrap),
  ]);
  assert.notEqual(first.binding.generation, second.binding.generation);
  assert.equal(first.binding.config.id, "config-1");
  assert.equal(second.binding.config.id, "config-2");
  assert.equal(first.matches(second.binding), false);
  assert.equal(
    (
      await port.submit(
        first.binding,
        fixtureCommand(),
        intent(first.binding),
        budget(),
      )
    ).certainty,
    "not_sent",
  );
  assert.equal(
    (
      await port.submit(
        second.binding,
        fixtureCommand(),
        intent(second.binding),
        budget(),
      )
    ).certainty,
    "submitted",
  );
});

test("each conformance operation owns a fresh non-aborted signal", async () => {
  const signals = new Set();
  await runProviderConformance(
    (scenario) => {
      const port = new ScriptedProvider();
      port.submission = scenario;
      for (const name of [
        "createSession",
        "submit",
        "cancel",
        "reconcile",
        "close",
      ]) {
        const original = port[name].bind(port);
        port[name] = (...args) => {
          const b = args.at(-1);
          assert.equal(b.signal.aborted, false);
          assert.equal(signals.has(b.signal), false);
          signals.add(b.signal);
          return original(...args);
        };
      }
      return port;
    },
    configuration,
    budget,
  );
  assert.ok(signals.size >= 8);
});

for (const operation of [
  "submit",
  "cancel",
  "reconcile",
  "observe-next",
  "observe-return",
  "close",
]) {
  test(`watchdog bounds uncooperative ${operation}`, async () => {
    const never = () => new Promise(() => {});
    const errors = [];
    const collect = (e) => {
      errors.push(e);
      for (const nested of e.errors ?? []) collect(nested);
    };
    const task = runProviderConformance(
      (scenario) => {
        const port = new ScriptedProvider();
        port.submission = scenario;
        if (operation === "observe-next" || operation === "observe-return") {
          port.observe = () => ({
            [Symbol.asyncIterator]: () => ({
              next:
                operation === "observe-next"
                  ? never
                  : async () => ({ done: true }),
              return:
                operation === "observe-return"
                  ? never
                  : async () => ({ done: true }),
            }),
          });
        } else port[operation] = never;
        return port;
      },
      configuration,
      () => ({ timeoutMs: 15, signal: new AbortController().signal }),
    );
    let timer;
    try {
      await assert.rejects(
        Promise.race([
          task,
          new Promise((_, reject) => {
            timer = setTimeout(() => reject(new Error("outer guard")), 1000);
          }),
        ]),
        (e) => {
          collect(e);
          return errors.some((error) => /budget exhausted/.test(error.message));
        },
      );
    } finally {
      clearTimeout(timer);
    }
  });
}

test("independent provider instances cannot accept each other's binding", async () => {
  const a = new ScriptedProvider(),
    b = new ScriptedProvider();
  const first = unwrap(await a.createSession(configuration, budget()));
  const second = unwrap(await b.createSession(configuration, budget()));
  assert.notDeepEqual(first.binding, second.binding);
  assert.equal(
    (
      await b.submit(
        first.binding,
        fixtureCommand(),
        intent(first.binding),
        budget(),
      )
    ).certainty,
    "not_sent",
  );
  assert.equal(
    (
      await b.submit(
        second.binding,
        fixtureCommand(),
        intent(second.binding),
        budget(),
      )
    ).certainty,
    "submitted",
  );
});

function intent(binding) {
  return {
    attemptId: "attempt-command-1",
    originGeneration: binding.generation,
    observerGeneration: binding.generation,
    nativeSessionId: binding.nativeSessionId,
    certainty: "intent",
  };
}
