import { acknowledge } from "../control.mjs";
import assert from "node:assert/strict";
import test from "node:test";
import { EventEmitter } from "node:events";
import { DeepSeekAdapter } from "../../../packages/ai-adapters/deepseek/dist/adapter.js";
import {
  COMPOSITION_ID,
  ACTIVE_PROFILE_ID,
} from "../../../packages/ai-adapters/deepseek/dist/assembly.js";
import { NativeFault } from "../../../packages/ai-adapters/deepseek/dist/protocol.js";
import { nativeRuntime } from "../../../packages/ai-adapters/deepseek/dist/runtime.js";
import {
  fixtureCommand,
  fixtureAttempt,
  unwrap,
} from "../../../packages/ai-contract/dist/testing/index.js";
import { configuration, budget } from "./support.mjs";
function fixture(answer = async () => ({})) {
  let notify,
    done,
    stops = 0;
  const diagnostics = [],
    c = configuration();
  const port = new DeepSeekAdapter(
    {
      clock: { now: () => 0 },
      onDiagnostic: (d) => diagnostics.push(d),
      resolveConfiguration: async () => ({
        configuration: c,
        persistenceDirectory: "/tmp/dsh",
        endpointIdentity: "https://custom.example.test/v1",
        apiUrl: "https://custom.example.test/v1",
        apiKey: "fixture",
        model: "deepseek-chat",
      }),
    },
    () => ({
      stopped: new Promise((r) => (done = r)),
      onEvent(fn) {
        notify = fn;
      },
      stop() {
        stops++;
        done();
      },
      async call(op, value) {
        if (op === "initialize")
          return {
            composition: COMPOSITION_ID,
            activation: ACTIVE_PROFILE_ID,
            nativeSessionId: value.nativeSessionId,
            observationOnly: false,
            previousTerminal: false,
          };
        if (op === "prompt")
          return {
            status: "accepted",
            requestId: value.requestId,
            activation: ACTIVE_PROFILE_ID,
            observationOnly: false,
          };
        if (op === "answer") return answer(value);
        return {};
      },
    }),
  );
  return { port, c, diagnostics, emit: (e) => notify(e), stops: () => stops };
}
async function question(f) {
  const opened = unwrap(await f.port.createSession(f.c, budget())),
    c = fixtureCommand();
  const sent = await f.port.dispatch(
    opened.binding,
    c,
    fixtureAttempt(opened.binding, c),
    budget(),
  );
  f.emit({
    type: "question",
    requestId: sent.binding.nativeRequestId,
    callbackId: "callback",
    request: {
      questions: [
        { id: "q", question: "Choose?", options: [{ label: "yes" }] },
      ],
    },
  });
  return {
    binding: sent.binding,
    response: {
      ...fixtureCommand("response"),
      input: {
        type: "respond",
        generation: sent.binding.generation,
        interactionId: "callback",
        answer: { answers: [{ id: "q", selected: ["yes"] }] },
      },
    },
  };
}
for (const reason of ["budget_exhausted", "process_exit", "protocol_failure"])
  test(`valid response ${reason} is not invalid_input`, async () => {
    const f = fixture(async () => {
      throw new NativeFault(reason);
    });
    try {
      const { binding, response } = await question(f);
      const result = await acknowledge(f.port, binding, response, budget());
      assert.equal(result.ok, false);
      assert.equal(result.error.code, "unavailable");
      assert.equal(result.error.retry, "reconcile_first");
    } finally {
      await f.port.close(budget());
    }
  });
test("abnormal lost stops once; expected close emits no process fault", async () => {
  const f = fixture();
  unwrap(await f.port.createSession(f.c, budget()));
  f.emit({
    type: "lost",
    diagnostic: { stage: "process", reason: "process_exit" },
  });
  f.emit({
    type: "lost",
    diagnostic: { stage: "process", reason: "process_exit" },
  });
  assert.equal(f.stops(), 1);
  assert.equal(f.diagnostics.length, 1);
  await f.port.close(budget());
  const expected = fixture();
  unwrap(await expected.port.createSession(expected.c, budget()));
  await expected.port.close(budget());
  expected.emit({
    type: "lost",
    diagnostic: { stage: "process", reason: "process_exit" },
  });
  assert.equal(expected.diagnostics.length, 0);
});
test("native reply failure retains its closed reason through bounded waiting", async () => {
  const child = new EventEmitter();
  child.pid = 42;
  child.kill = () => {
    queueMicrotask(() => child.emit("close", 0));
    return true;
  };
  child.send = (frame, cb) => {
    cb?.();
    queueMicrotask(() =>
      child.emit("message", {
        type: "reply",
        id: frame.id,
        ok: false,
        reason: "restoration_failed",
      }),
    );
  };
  const runtime = nativeRuntime(() => child);
  try {
    await assert.rejects(
      runtime.call("inspect", { requestId: "request" }, budget()),
      (e) => e instanceof NativeFault && e.reason === "restoration_failed",
    );
  } finally {
    runtime.stop();
    await runtime.stopped;
  }
});

test("fixed IPC codec rejects wrong request, result, envelope and secret-bearing reason", async () => {
  const { decodeRequest, decodeResponse, decodeFrame } = await import(
    "../../../packages/ai-adapters/deepseek/dist/protocol.js"
  );
  for (const call of [
    () =>
      decodeRequest({ id: 1, operation: "inspect", value: { text: "wrong" } }),
    () => decodeRequest({ id: 1, operation: "activate", value: {} }),
    () =>
      decodeResponse("prompt", { status: "terminal", outcome: "completed" }),
    () =>
      decodeResponse("inspect", { status: "terminal", outcome: "invented" }),
    () =>
      decodeFrame({
        type: "reply",
        id: 1,
        ok: false,
        reason: "provider secret",
      }),
    () =>
      decodeFrame({
        type: "event",
        event: {
          type: "proposal",
          requestId: "r",
          callbackId: "cb",
          proposal: { name: "n", arguments: "wrong" },
        },
      }),
  ])
    assert.throws(
      call,
      (error) =>
        error instanceof NativeFault &&
        error.reason === "protocol_failure" &&
        !error.message.includes("secret"),
    );
});

test("closing during an answer suppresses the expected late transport fault", async () => {
  let reject;
  const f = fixture(() => new Promise((_, r) => (reject = r)));
  const { binding, response } = await question(f);
  const pending = acknowledge(f.port, binding, response, budget());
  await f.port.close(budget());
  reject(new NativeFault("process_exit"));
  assert.equal((await pending).ok, false);
  assert.deepEqual(f.diagnostics, []);
});
