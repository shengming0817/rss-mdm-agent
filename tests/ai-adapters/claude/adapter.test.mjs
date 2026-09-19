import {
  fixtureAttempt,
  fixtureDispatchedRecord,
  fixtureProviderSession,
  fixtureSession,
} from "../../../packages/ai-contract/dist/testing/index.js";
import assert from "node:assert/strict";
import { test } from "node:test";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { pathToFileURL } from "node:url";
import { createTestAdapter } from "../../../packages/ai-adapters/claude/dist/testing.js";
import {
  fixtureCommand,
  unwrap,
  runProviderConformance,
} from "../../../packages/ai-contract/dist/testing/index.js";
const configuration = {
  provider: "claude",
  config: { id: "claude-config", revision: "1" },
  accountRef: "account-1",
  workingDirectory: "/tmp",
  namespace: fixtureSession().namespace,
  permissions: "tools_disabled",
};
const budget = (ms = 500) => ({
  timeoutMs: ms,
  signal: new AbortController().signal,
});
class Queue {
  items = [];
  waiters = [];
  ended = false;
  push(value) {
    const w = this.waiters.shift();
    if (w) w({ value, done: false });
    else this.items.push(value);
  }
  end() {
    this.ended = true;
    for (const w of this.waiters.splice(0)) w({ done: true });
  }
  [Symbol.asyncIterator]() {
    return this;
  }
  next() {
    if (this.items.length)
      return Promise.resolve({ value: this.items.shift(), done: false });
    if (this.ended) return Promise.resolve({ done: true });
    return new Promise((r) => this.waiters.push(r));
  }
}
function harness({
  acknowledge = true,
  stopOnClose = true,
  conformance = false,
  clock = () => 0,
  ttl = 120000,
} = {}) {
  let options,
    input,
    stopped,
    lastUser,
    closed = 0,
    interrupted = 0;
  const output = new Queue();
  const adapter = createTestAdapter(
    {
      resolveConfiguration: async () => ({
        configuration,
        configurationDirectory: "/tmp/rss-sdk-test-config",
        apiUrl: "https://example.invalid",
        credential: { type: "api_key", value: "fixture-secret" },
      }),
      clock: { now: clock },
      callbackTimeoutMs: ttl,
    },
    (o, prompts) => {
      options = o;
      input = prompts[Symbol.asyncIterator]();
      const native = Object.assign(output, {
        initializationResult: async () => ({}),
        interrupt: async () => {
          interrupted++;
          if (conformance) output.end();
        },
        close: () => {
          closed++;
          output.end();
          if (stopOnClose) stopped();
        },
      });
      const stop = new Promise((r) => {
        stopped = r;
      });
      (async () => {
        for (;;) {
          const item = await input.next();
          if (item.done) break;
          const user = item.value;
          lastUser = user;
          if (acknowledge) {
            output.push({
              type: "system",
              subtype: "init",
              session_id: user.session_id,
              claude_code_version: "2.1.277",
              tools: ["AskUserQuestion"],
              mcp_servers: [],
              skills: [],
              plugins: [],
              permissionMode: "default",
            });
            output.push(user);
          } else if (conformance) output.end();
        }
      })();
      return { query: native, stopped: stop };
    },
  );
  return {
    adapter,
    output,
    get lastUser() {
      return lastUser;
    },
    get options() {
      return options;
    },
    get interrupted() {
      return interrupted;
    },
    get closed() {
      return closed;
    },
    stop: () => stopped(),
  };
}
const create = async (h) =>
  unwrap(await h.adapter.createSession(configuration, budget())).binding;
test("explicit native session and restricted SDK configuration; close proves exit", async () => {
  const h = harness();
  const b = await create(h);
  assert.match(b.nativeSessionId, /^[a-f0-9-]{36}$/);
  assert.equal(h.options.sessionId, b.nativeSessionId);
  assert.equal(h.options.continue, undefined);
  assert.deepEqual(h.options.tools, ["AskUserQuestion"]);
  assert.deepEqual(h.options.settingSources, []);
  assert.deepEqual(h.options.plugins, []);
  assert.equal(h.options.strictMcpConfig, true);
  assert.equal(h.options.permissionMode, "default");
  assert.equal(h.options.env.ANTHROPIC_API_KEY, "fixture-secret");
  assert.equal(h.options.env.ANTHROPIC_AUTH_TOKEN, undefined);
  assert.equal(unwrap(await h.adapter.close(budget())).processStopped, true);
});
test("submit waits for native acceptance; deltas and terminal retain prompt identity", async () => {
  const h = harness(),
    b = await create(h),
    c = fixtureCommand();
  const submitted = await h.adapter.submit(
    b,
    c,
    fixtureAttempt(b, c),
    budget(),
  );
  assert.equal(submitted.certainty, "submitted");
  const live = submitted.binding;
  assert.ok(live.nativeRequestId);
  assert.equal(live.nativeRunId, undefined);
  h.output.push({
    type: "assistant",
    session_id: b.nativeSessionId,
    uuid: "message-1",
    parent_tool_use_id: null,
    message: { id: "msg-1", content: [{ type: "text", text: "ok" }] },
  });
  h.output.push({
    type: "result",
    subtype: "success",
    is_error: false,
    session_id: b.nativeSessionId,
    user_message_uuid: live.nativeRequestId,
  });
  const events = await Array.fromAsync(h.adapter.observe(live, budget()));
  assert.ok(events.some((x) => x.body?.text === "ok"));
  assert.equal(events.at(-1).body.outcome, "completed");
  assert.ok(
    events.every(
      (x) =>
        x.commandId === c.commandId &&
        x.binding.nativeRequestId === live.nativeRequestId,
    ),
  );
  assert.equal(
    (await h.adapter.submit(b, c, fixtureAttempt(b, c), budget())).certainty,
    "submitted",
  );
  await h.adapter.close(budget());
});
test("ambiguous send and cancellation cannot manufacture a terminal", async () => {
  const h = harness({ acknowledge: false }),
    b = await create(h),
    c = fixtureCommand();
  assert.equal(
    (await h.adapter.submit(b, c, fixtureAttempt(b, c), budget(15))).certainty,
    "unknown",
  );
  const cancel = {
    ...c,
    commandId: "cancel-1",
    input: {
      type: "cancel",
      targetCommandId: c.commandId,
      generation: b.generation,
    },
  };
  assert.equal(
    unwrap(await h.adapter.cancel(b, cancel, budget())),
    "request_only",
  );
  assert.equal(h.interrupted, 1);
  const events = await Array.fromAsync(h.adapter.observe(b, budget(15)));
  assert.equal(
    events.some((x) => x.body?.type === "terminal"),
    false,
  );
  await h.adapter.close(budget());
});
test("two native callbacks under one prompt are answered independently and only once", async () => {
  const h = harness(),
    b = await create(h),
    c = fixtureCommand(),
    sent = await h.adapter.submit(b, c, fixtureAttempt(b, c), budget()),
    live = sent.binding;
  const questions = {
    questions: [
      {
        question: "Choose?",
        header: "Choice",
        options: [
          { label: "A", description: "a" },
          { label: "B", description: "b" },
        ],
        multiSelect: false,
      },
    ],
  };
  const cb = (requestId, toolUseID) =>
    h.options.canUseTool("AskUserQuestion", questions, {
      requestId,
      toolUseID,
      signal: new AbortController().signal,
    });
  const first = cb("callback-1", "tool-1"),
    second = cb("callback-2", "tool-2");
  const it = h.adapter.observe(live, budget())[Symbol.asyncIterator]();
  const seen = [];
  while (seen.length < 2) {
    const x = await it.next();
    if (x.value.type === "interaction") seen.push(x.value.interaction);
  }
  assert.deepEqual(
    seen.map((x) => x.nativeCallbackId),
    ["callback-1", "callback-2"],
  );
  const respond = (q, id) => ({
    ...c,
    commandId: id,
    input: {
      type: "respond",
      interactionId: q.interactionId,
      generation: live.generation,
      answer: { answers: { "Choose?": "A" } },
    },
  });
  const reply = respond(seen[1], "response-2");
  assert.equal((await h.adapter.respond(live, reply, budget())).ok, true);
  assert.equal((await second).behavior, "allow");
  assert.equal((await h.adapter.respond(live, reply, budget())).ok, true);
  assert.equal(
    (
      await h.adapter.respond(
        live,
        respond(seen[1], "response-other"),
        budget(),
      )
    ).error.code,
    "already_answered",
  );
  assert.equal(
    (await h.adapter.respond(live, respond(seen[0], "response-1"), budget()))
      .ok,
    true,
  );
  assert.equal((await first).behavior, "allow");
  await it.return();
  await h.adapter.close(budget());
});
test("execution and foreign MCP requests always fail closed", async () => {
  const h = harness(),
    b = await create(h);
  await h.adapter.submit(
    b,
    fixtureCommand(),
    fixtureAttempt(b, fixtureCommand()),
    budget(),
  );
  for (const tool of [
    "Bash",
    "Read",
    "Write",
    "Agent",
    "mcp__rss_host__propose",
  ]) {
    const result = await h.options.canUseTool(
      tool,
      { command: "unsafe" },
      {
        requestId: "callback-x",
        toolUseID: "tool-x",
        signal: new AbortController().signal,
        mcpServer: { name: "rss_host", source: "project" },
      },
    );
    assert.equal(result.behavior, "deny");
    const hook = await h.options.hooks.PreToolUse[0].hooks[0](
      {
        hook_event_name: "PreToolUse",
        tool_name: tool,
        tool_input: {},
        tool_use_id: "x",
        session_id: b.nativeSessionId,
      },
      "x",
      { signal: new AbortController().signal },
    );
    assert.equal(hook.hookSpecificOutput.permissionDecision, "deny");
  }
  await h.adapter.close(budget());
});
test("stale binding and steer cannot dispatch; close timeout stays retryable", async () => {
  const h = harness({ stopOnClose: false }),
    b = await create(h),
    c = fixtureCommand();
  assert.equal(
    (
      await h.adapter.submit(
        { ...b, accountRef: "other" },
        c,
        fixtureAttempt({ ...b, accountRef: "other" }, c),
        budget(),
      )
    ).certainty,
    "not_sent",
  );
  assert.equal(
    (
      await h.adapter.submit(
        b,
        { ...c, input: { ...c.input, policy: "steer", targetRunId: "run" } },
        fixtureAttempt(b, {
          ...c,
          input: { ...c.input, policy: "steer", targetRunId: "run" },
        }),
        budget(),
      )
    ).error.code,
    "unsupported_capability",
  );
  const closed = await h.adapter.close(budget(15));
  assert.equal(closed.ok, false);
  h.stop();
  assert.equal(unwrap(await h.adapter.close(budget())).processStopped, true);
});

test("shared A01 provider conformance runs against the SDK fixture seam", () =>
  runProviderConformance(
    (scenario) =>
      harness({ acknowledge: scenario === "submitted", conformance: true })
        .adapter,
    configuration,
    () => budget(1000),
  ));
for (const loss of ["abort", "expiry", "close"])
  test(`pending callback rejects response after ${loss}`, async () => {
    let now = 0;
    const h = harness({ clock: () => now, ttl: 100 }),
      binding = await create(h),
      sent = await h.adapter.submit(
        binding,
        fixtureCommand(),
        fixtureAttempt(binding, fixtureCommand()),
        budget(),
      ),
      live = sent.binding;
    const controller = new AbortController();
    const question = h.options.canUseTool(
      "AskUserQuestion",
      {
        questions: [
          {
            question: "Choose?",
            header: "Choice",
            options: [
              { label: "A", description: "a" },
              { label: "B", description: "b" },
            ],
            multiSelect: false,
          },
        ],
      },
      {
        requestId: "callback-loss",
        toolUseID: "tool-loss",
        signal: controller.signal,
      },
    );
    const it = h.adapter.observe(live, budget())[Symbol.asyncIterator]();
    let pending;
    do {
      pending = (await it.next()).value;
    } while (pending.type !== "interaction");
    const response = {
      ...fixtureCommand("answer"),
      input: {
        type: "respond",
        interactionId: pending.interaction.interactionId,
        generation: live.generation,
        answer: { answers: { "Choose?": "A" } },
      },
    };
    if (loss === "abort") controller.abort();
    else if (loss === "expiry") now = 101;
    else await h.adapter.close(budget());
    assert.equal((await h.adapter.respond(live, response, budget())).ok, false);
    assert.equal((await question).behavior, "deny");
    await it.return();
    await h.adapter.close(budget());
  });
test("question response cannot carry permission or cross-account authority", async () => {
  const h = harness(),
    binding = await create(h),
    sent = await h.adapter.submit(
      binding,
      fixtureCommand(),
      fixtureAttempt(binding, fixtureCommand()),
      budget(),
    ),
    live = sent.binding;
  const result = h.options.canUseTool(
    "AskUserQuestion",
    {
      questions: [
        {
          question: "Choose?",
          header: "Choice",
          options: [
            { label: "A", description: "a" },
            { label: "B", description: "b" },
          ],
          multiSelect: false,
        },
      ],
    },
    {
      requestId: "callback-security",
      toolUseID: "tool-security",
      signal: new AbortController().signal,
    },
  );
  const it = h.adapter.observe(live, budget())[Symbol.asyncIterator]();
  let pending;
  do {
    pending = (await it.next()).value;
  } while (pending.type !== "interaction");
  const command = {
    ...fixtureCommand("answer"),
    input: {
      type: "respond",
      interactionId: pending.interaction.interactionId,
      generation: live.generation,
      answer: {
        answers: { "Choose?": "A" },
        approved: true,
        updatedPermissions: [{ type: "setMode", mode: "bypassPermissions" }],
      },
    },
  };
  assert.equal(
    (await h.adapter.respond(live, command, budget())).error.code,
    "invalid_input",
  );
  delete command.input.answer.approved;
  delete command.input.answer.updatedPermissions;
  assert.equal(
    (
      await h.adapter.respond(
        { ...live, accountRef: "foreign" },
        command,
        budget(),
      )
    ).ok,
    false,
  );
  assert.equal(
    (
      await h.adapter.respond(
        live,
        { ...command, input: { ...command.input, generation: "stale" } },
        budget(),
      )
    ).ok,
    false,
  );
  await it.return();
  await h.adapter.close(budget());
  assert.equal((await result).behavior, "deny");
});
test("foreign native terminal and missing prompt correlation never complete a turn", async () => {
  for (const patch of [
    { session_id: "foreign" },
    { user_message_uuid: "foreign" },
    { user_message_uuid: undefined },
  ]) {
    const h = harness(),
      binding = await create(h),
      sent = await h.adapter.submit(
        binding,
        fixtureCommand(),
        fixtureAttempt(binding, fixtureCommand()),
        budget(),
      );
    h.output.push({
      type: "result",
      subtype: "success",
      is_error: false,
      session_id: binding.nativeSessionId,
      user_message_uuid: sent.binding.nativeRequestId,
      ...patch,
    });
    const observations = await Array.fromAsync(
      h.adapter.observe(sent.binding, budget()),
    );
    assert.equal(
      observations.some((x) => x.body?.type === "terminal"),
      false,
    );
    await h.adapter.close(budget());
  }
});

test("exhausted budgets do not send prompts or interrupts", async () => {
  const h = harness(),
    binding = await create(h),
    cmd = fixtureCommand();
  assert.equal(
    (
      await h.adapter.submit(
        binding,
        cmd,
        fixtureAttempt(binding, cmd),
        budget(0),
      )
    ).certainty,
    "not_sent",
  );
  const sent = await h.adapter.submit(
    binding,
    cmd,
    fixtureAttempt(binding, cmd),
    budget(),
  );
  assert.equal(sent.certainty, "submitted");
  const cancel = {
    ...fixtureCommand("cancel"),
    input: {
      type: "cancel",
      targetCommandId: cmd.commandId,
      generation: binding.generation,
    },
  };
  const abort = new AbortController();
  abort.abort();
  assert.equal(
    (
      await h.adapter.cancel(sent.binding, cancel, {
        timeoutMs: 100,
        signal: abort.signal,
      })
    ).ok,
    false,
  );
  assert.equal(h.interrupted, 0);
  await h.adapter.close(budget());
});

test("invalid resume input returns a value-free Result", async () => {
  const h = harness();
  const result = await h.adapter.resume(
    {
      provider: "claude",
      config: { id: "c", revision: "1" },
      accountRef: "account",
      nativeSessionId: "bad",
      generation: "bad",
      nativeRequestId: undefined,
    },
    configuration,
    budget(),
  );
  assert.equal(result.ok, false);
  await h.adapter.close(budget());
});

test("late native acceptance reconciles unknown submission to running", async () => {
  const h = harness({ acknowledge: false }),
    binding = await create(h),
    cmd = fixtureCommand();
  assert.equal(
    (
      await h.adapter.submit(
        binding,
        cmd,
        fixtureAttempt(binding, cmd),
        budget(15),
      )
    ).certainty,
    "unknown",
  );
  h.output.push({
    type: "system",
    subtype: "init",
    session_id: binding.nativeSessionId,
    claude_code_version: "2.1.277",
    tools: ["AskUserQuestion"],
    mcp_servers: [],
    skills: [],
    plugins: [],
    permissionMode: "default",
  });
  h.output.push(h.lastUser);
  await new Promise((r) => setImmediate(r));
  const state = unwrap(
    await h.adapter.reconcile(
      binding,
      fixtureDispatchedRecord(binding, cmd, configuration.namespace),
      budget(),
    ),
  );
  assert.equal(state.status, "running");
  assert.equal(state.binding.nativeRequestId, h.lastUser.uuid);
  assert.equal(
    (
      await h.adapter.submit(
        binding,
        cmd,
        fixtureAttempt(binding, cmd),
        budget(),
      )
    ).certainty,
    "submitted",
  );
  await h.adapter.close(budget());
});

test("binding compatibility identity matches installed package manifests", async () => {
  const entry = import.meta.resolve(
    "../../../packages/ai-adapters/claude/dist/index.js",
  );
  const manifest = (url) => JSON.parse(readFileSync(url, "utf8"));
  const own = manifest(new URL("../package.json", entry));
  const sdk = manifest(
    new URL(
      "./package.json",
      pathToFileURL(
        createRequire(entry).resolve("@anthropic-ai/claude-agent-sdk"),
      ),
    ),
  );
  const h = harness(),
    binding = await create(h);
  assert.equal(binding.adapterVersion, own.version);
  assert.equal(
    binding.providerVersion,
    `claude-agent-sdk-${sdk.version}/claude-code-${sdk.claudeCodeVersion}`,
  );
  await h.adapter.close(budget());
});

for (const consume of [false, true])
  test(`session display budget ${consume ? "is released by consumption" : "bounds unconsumed turns"}`, async () => {
    const h = harness();
    let binding = await create(h);
    for (let n = 0; n < 4; n++) {
      const sent = await h.adapter.submit(
        binding,
        fixtureCommand(`large-${n}`),
        fixtureAttempt(binding, fixtureCommand(`large-${n}`)),
        budget(),
      );
      assert.equal(sent.certainty, "submitted");
      binding = sent.binding;
      for (let i = 0; i < 12; i++)
        h.output.push({
          type: "assistant",
          session_id: binding.nativeSessionId,
          uuid: `large-${i}`,
          parent_tool_use_id: null,
          message: {
            id: `msg-${i}`,
            content: [{ type: "text", text: "x".repeat(100000) }],
          },
        });
      h.output.push({
        type: "result",
        subtype: "success",
        is_error: false,
        session_id: binding.nativeSessionId,
        user_message_uuid: binding.nativeRequestId,
      });
      await new Promise((r) => setImmediate(r));
      if (consume) await Array.fromAsync(h.adapter.observe(binding, budget()));
    }
    assert.equal(h.closed > 0, !consume);
    const state = unwrap(
      await h.adapter.reconcile(
        binding,
        fixtureDispatchedRecord(
          binding,
          fixtureCommand("large-3"),
          configuration.namespace,
        ),
        budget(),
      ),
    );
    assert.equal(state.status, consume ? "terminal" : "unknown");
    await h.adapter.close(budget());
  });

test("new and resumed sessions cannot omit configuration identity fields", async () => {
  const original = harness();
  const prior = await create(original);
  await original.adapter.close(budget());
  for (const operation of ["createSession", "resume"]) {
    const h = harness(),
      malformed = { ...configuration };
    delete malformed.workingDirectory;
    try {
      const result =
        operation === "resume"
          ? await h.adapter.resume(prior, malformed, budget())
          : await h.adapter.createSession(malformed, budget());
      assert.equal(result.ok, false);
      assert.equal(h.options, undefined, "reject before starting a runtime");
    } finally {
      await h.adapter.close(budget());
    }
  }
});

for (const [patch, outcome] of [
  [{ terminal_reason: "aborted_streaming" }, "cancelled"],
  [{ terminal_reason: "aborted_tools" }, "cancelled"],
  [{ stop_reason: "refusal" }, "refused"],
  [{ stop_reason: "max_tokens" }, "max_tokens"],
  [{ subtype: "error_max_turns", is_error: true }, "max_turn_requests"],
  [{ subtype: "error_max_budget_usd", is_error: true }, "failed"],
  [
    { subtype: "error_max_structured_output_retries", is_error: true },
    "failed",
  ],
])
  test(`native terminal maps to the closed ACP-compatible outcome: ${JSON.stringify(patch)}`, async () => {
    const h = harness();
    try {
      const session = unwrap(
        await h.adapter.createSession(configuration, budget()),
      );
      assert.equal(session.capabilities.queue, "unsupported");
      const sent = await h.adapter.submit(
        session.binding,
        fixtureCommand(),
        fixtureAttempt(session.binding, fixtureCommand()),
        budget(),
      );
      assert.equal(sent.certainty, "submitted");
      h.output.push({
        type: "result",
        subtype: "success",
        is_error: false,
        session_id: session.binding.nativeSessionId,
        user_message_uuid: sent.binding.nativeRequestId,
        ...patch,
      });
      const events = await Array.fromAsync(
        h.adapter.observe(sent.binding, budget()),
      );
      assert.equal(events.at(-1).body.outcome, outcome);
    } finally {
      await h.adapter.close(budget());
    }
  });
test("dispatch and reconciliation cannot substitute another attempt or namespace", async () => {
  const h = harness(),
    binding = await create(h),
    command = fixtureCommand();
  const attempt = fixtureAttempt(binding, command);
  try {
    assert.equal(
      (await h.adapter.submit(binding, command, attempt, budget())).certainty,
      "submitted",
    );
    assert.equal(
      (
        await h.adapter.submit(
          binding,
          command,
          { ...attempt, attemptId: "forged-attempt" },
          budget(),
        )
      ).certainty,
      "not_sent",
    );
    const record = fixtureDispatchedRecord(
      binding,
      command,
      configuration.namespace,
    );
    for (const altered of [
      {
        ...record,
        dispatch: { ...record.dispatch, attemptId: "forged-attempt" },
      },
      {
        ...record,
        receipt: {
          ...record.receipt,
          namespace: { ...record.receipt.namespace, tenantId: "other" },
        },
      },
    ])
      assert.equal(
        (await h.adapter.reconcile(binding, altered, budget())).error.code,
        "stale_binding",
      );
  } finally {
    await h.adapter.close(budget());
  }
});
