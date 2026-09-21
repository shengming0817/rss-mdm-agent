import { mount, flushPromises } from "@vue/test-utils";
import Connections from "./Connections.vue";
import { activeStage } from "@rss-mdm-agent/ai-contract";
import { describe, expect, it, vi } from "vitest";
import {
  ClientError,
  type ClientOptions,
  type RuntimeClient,
  type SessionView,
} from "@rss-mdm-agent/ai-client";
import { createAssistant, operationMessage } from "./controller";
import { fixtureSession } from "@rss-mdm-agent/ai-contract/testing";
import fixtures from "../../../../tests/assistant/execution-fixtures.json";
import type { ExecutionTaskDetails } from "./execution-types";
function setup(now = () => 100) {
  let next = 0,
    listener: (view: SessionView) => void = () => {},
    options: ClientOptions = {};
  const session = fixtureSession(),
    view: SessionView = {
      namespace: session.namespace,
      selectedConnectionId: "config-1",
      generation: activeStage(session).binding.generation,
      cursor: 0,
      capabilities: activeStage(session).capabilities,
      sessionStatus: session.status,
      timeline: [],
      connection: "attached",
      commands: {},
      messages: {},
      interactions: {},
      surfaces: {},
      tools: {},
    };
  view.connection = "attached";
  const submit = vi.fn().mockResolvedValue({ kind: "receipt" });
  const client = {
    initialize: vi.fn().mockResolvedValue({ contractVersion: 5, acp: 1 }),
    connections: vi.fn().mockResolvedValue({
      preferences: { schemaVersion: 5, kind: "userPreferences" },
      connections: [
        {
          schemaVersion: 5,
          kind: "connection",
          connectionId: "config-1",
          name: "Fixture",
          provider: "codex",
          configRevision: 1,

          profile: "conversation",
          status: "ready",
          source: { type: "existing_config", directory: "/fixture" },
        },
      ],
    }),
    savePreferences: vi
      .fn()
      .mockResolvedValue({ schemaVersion: 5, kind: "userPreferences" }),
    listSessions: vi.fn().mockResolvedValue({ items: [session] }),
    restore: vi.fn().mockResolvedValue(view),
    createSession: vi.fn().mockResolvedValue(view),
    submit,
    observe(fn: typeof listener) {
      listener = fn;
      return () => {
        listener = () => {};
      };
    },
    close: vi.fn(),
    connection: { closed: new Promise(() => {}) },
    resume: vi.fn().mockResolvedValue(view),
  } as unknown as RuntimeClient;
  const taskDetails = vi.fn();
  const c = createAssistant(
    {
      connect: async (input) => {
        options = input;
        return { runtime: client, mode: "s1" };
      },
      taskDetails,
    },
    () => `id-${++next}`,
    now,
  );
  return {
    c,
    view,
    client,
    submit,
    taskDetails,
    options: () => options,
    emit: (v = view) => listener(structuredClone(v)),
  };
}
describe("assistant application ownership", () => {
  it("queues through Host while provider capabilities independently gate steer, cancel and resume", async () => {
    const t = setup();
    await t.c.connect();
    await t.c.select("session-1");
    t.view.commands.p = {
      command: {
        schemaVersion: 5,
        kind: "command",
        sessionId: "session-1",
        commandId: "p",
        expiresAtMs: 1000,
        input: { type: "prompt", policy: "queue_next", text: "first" },
      },
      state: "running",
      dispatch: {
        attemptId: "attempt",
        originGeneration: "generation-1",
        observerGeneration: "generation-1",
        nativeSessionId: "native-1",
        nativeRunId: "run-1",
        certainty: "submitted",
      },
    };
    t.emit();
    t.c.draft.value = "editable while busy";
    expect(t.c.busy.value).toBe(true);
    expect(t.c.canSend.value).toBe(true);
    expect(t.c.canSteer.value).toBe(false);
    expect(t.c.canResume.value).toBe(false);
    await t.c.prompt();
    expect(t.submit.mock.calls[0][0].input.policy).toBe("queue_next");
    t.submit.mockClear();
    t.c.draft.value = "editable while busy";
    t.view.capabilities.steer = "supported";
    t.emit();
    await t.c.prompt("steer");
    expect(t.submit.mock.calls[0][0].input).toEqual({
      type: "prompt",
      policy: "steer",
      text: "editable while busy",
      targetRunId: "run-1",
    });
    t.view.capabilities.cancellation = "unknown";
    t.emit();
    await t.c.cancel();
    expect(t.submit).toHaveBeenCalledTimes(1);
    t.view.commands.p.state = "reconciliation_required";
    t.view.commands.p.dispatch!.certainty = "unknown";
    t.view.capabilities.continuation = "across_processes";
    t.emit();
    expect(t.c.canSteer.value).toBe(false);
    expect(t.c.canCancel.value).toBe(false);
    expect(t.c.canResume.value).toBe(true);
    for (const settled of ["acknowledged", "cancelled"] as const) {
      t.view.commands.p.state = settled;
      t.emit();
      expect(t.c.busy.value).toBe(false);
      expect(t.c.canSteer.value).toBe(false);
      expect(t.c.canCancel.value).toBe(false);
    }
    t.c.dispose();
  });
  it("does not turn receipts into history and retries exactly the unknown command without erasing a newer draft", async () => {
    const t = setup();
    await t.c.connect();
    await t.c.select("session-1");
    t.submit.mockRejectedValueOnce(new ClientError("request_failed"));
    t.c.draft.value = "original";
    await t.c.prompt();
    const original = JSON.stringify(t.submit.mock.calls[0][0]);
    expect(t.c.view.value?.timeline).toEqual([]);
    t.c.draft.value = "newer edit";
    await t.c.retry();
    expect(JSON.stringify(t.submit.mock.calls[1][0])).toBe(original);
    expect(t.c.draft.value).toBe("newer edit");
    expect(t.c.state.pending.size).toBe(0);
    t.c.dispose();
  });
  it("routes an answer using the original question generation/run and rejects unavailable or surface-bound questions", async () => {
    const t = setup();
    await t.c.connect();
    await t.c.select("session-1");
    t.view.capabilities.structuredQuestion = "supported";
    t.view.interactions.q = {
      commandId: "original",
      generation: t.view.generation,
      nativeRunId: "original-run",
      status: "pending",
      expiresAtMs: 1000,
      callbackLifetime: "generation_bound",
      request: {},
    };
    t.emit();
    await t.c.respond("q", { choice: "yes" });
    expect(t.submit.mock.calls[0][0].input).toEqual({
      type: "respond",
      interactionId: "q",
      generation: "generation-1",
      nativeRunId: "original-run",
      answer: { answers: { choice: "yes" } },
    });
    t.view.interactions.q.status = "unavailable";
    t.emit();
    await t.c.respond("q", { choice: "yes" });
    expect(t.submit).toHaveBeenCalledTimes(1);
    t.view.interactions.q.status = "pending";
    t.view.interactions.q.expiresAtMs = 99;
    t.emit();
    expect(t.c.answerable("q")).toBe(false);
    t.view.interactions.q.expiresAtMs = 1000;
    t.view.interactions.q.generation = "old-generation";
    t.emit();
    expect(t.c.answerable("q")).toBe(false);
    t.c.dispose();
  });
  it("closes standard permission prompts on AbortSignal and rejects forged option identifiers", async () => {
    const t = setup();
    await t.c.connect();
    const signal = new AbortController();
    const result = t.options().requestPermission!(
      {
        sessionId: "session-1",
        toolCall: { toolCallId: "call", title: "tool" },
        options: [
          { optionId: "known", name: "Allow once", kind: "allow_once" },
        ],
      },
      signal.signal,
    );
    const id = [...t.c.state.permissions.keys()][0];
    t.c.permission(id, "forged");
    expect(t.c.state.permissions.size).toBe(1);
    signal.abort();
    expect(await result).toEqual({ outcome: { outcome: "cancelled" } });
    expect(t.c.state.permissions.size).toBe(0);
    t.c.dispose();
  });
  it("clears all caller display and pending callbacks on a new authenticated connection", async () => {
    const t = setup();
    await t.c.connect();
    await t.c.select("session-1");
    t.c.draft.value = "private caller draft";
    const result = t.options().requestPermission!(
      {
        sessionId: "session-1",
        toolCall: { toolCallId: "call", title: "tool" },
        options: [],
      },
      new AbortController().signal,
    );
    await t.c.connect();
    expect(t.c.state.views.size).toBe(0);
    expect(t.c.draft.value).toBe("");
    expect(await result).toEqual({ outcome: { outcome: "cancelled" } });
    t.c.dispose();
  });
  it("retains only the latest authorized execution read and clears it on a failed read", async () => {
    const t = setup();
    let release: (v: ExecutionTaskDetails) => void = () => {};
    t.taskDetails
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            release = resolve;
          }),
      )
      .mockResolvedValueOnce(fixtures.outcomeUnknown)
      .mockRejectedValueOnce(new Error("secret server text"));
    const old = t.c.taskDetails("old");
    await t.c.taskDetails("current");
    release(fixtures.running as ExecutionTaskDetails);
    await old;
    expect(t.c.state.task?.status.phase).toBe("outcomeUnknown");
    await t.c.taskDetails("forbidden");
    expect(t.c.state.task).toBeUndefined();
    expect(t.c.state.taskError).toBe("无法读取授权执行详情");
    t.c.dispose();
  });
  it("clears only each recovered operation's diagnosis", async () => {
    const t = setup();
    await t.c.connect();
    vi.mocked(t.client.listSessions).mockRejectedValueOnce(
      new ClientError("unavailable"),
    );
    vi.mocked(t.client.createSession).mockRejectedValueOnce(
      new ClientError("invalid_input"),
    );
    await t.c.list(false);
    await t.c.create();
    expect(t.c.state.listError).toBe("unavailable");
    expect(t.c.state.createError).toBe("invalid_input");
    await t.c.list(false);
    expect(t.c.state.listError).toBe("");
    expect(t.c.state.createError).toBe("invalid_input");
    await t.c.create();
    expect(t.c.state.createError).toBe("");
    t.c.state.views.clear();
    vi.mocked(t.client.restore).mockRejectedValueOnce(
      new ClientError("unavailable"),
    );
    await t.c.select("session-1");
    expect(t.c.state.errors.get("session-1")).toBe("unavailable");
    await t.c.select("session-1");
    expect(t.c.state.errors.has("session-1")).toBe(false);
    t.c.dispose();
  });
  it("finishes reconnect and disposal even if the transport disposer throws", async () => {
    const t = setup();
    await t.c.connect();
    await t.c.select("session-1");
    t.c.draft.value = "private";
    const pending = t.options().requestPermission!(
      {
        sessionId: "session-1",
        toolCall: { toolCallId: "call", title: "tool" },
        options: [],
      },
      new AbortController().signal,
    );
    vi.mocked(t.client.close).mockImplementation(() => {
      throw new Error("private raw failure");
    });
    await t.c.connect();
    expect(await pending).toEqual({ outcome: { outcome: "cancelled" } });
    expect(t.c.state.views.size).toBe(0);
    expect(t.c.draft.value).toBe("");
    expect(t.c.state.cleanupError).toBe("cleanup_failed");
    expect(() => t.c.dispose()).not.toThrow();
    expect(t.c.runtime.value).toBeUndefined();
    expect(t.c.state.connection).toBe("disconnected");
  });
  it("distinguishes detach, active runs, retired sessions and unavailable continuation", async () => {
    const t = setup();
    await t.c.connect();
    await t.c.select("session-1");
    t.view.capabilities.continuation = "unknown";
    t.emit();
    expect(t.c.resumeReason.value).toBe("unknown");
    t.view.capabilities.continuation = "unsupported";
    t.emit();
    expect(t.c.resumeReason.value).toBe("unsupported");
    t.view.capabilities.continuation = "across_processes";
    t.view.connection = "detached";
    t.emit();
    expect(t.c.sessionConnection.value).toBe("detached");
    expect(t.c.resumeReason.value).toBe("not-attached");
    t.view.connection = "attached";
    t.view.sessionStatus = "retired";
    t.emit();
    expect(t.c.resumeReason.value).toBe("retired");
    t.c.dispose();
  });
});

it("rejects unknown permission kinds before exposing a callback", async () => {
  const t = setup();
  await t.c.connect();
  const result = t.options().requestPermission!(
    {
      sessionId: "session-1",
      toolCall: { toolCallId: "tool", title: "tool" },
      options: [
        { optionId: "unknown", name: "Allow once", kind: "unknown" as never },
      ],
    },
    new AbortController().signal,
  );
  try {
    expect(t.c.state.permissions.size).toBe(0);
  } finally {
    t.c.dispose();
  }
  expect(await result).toEqual({ outcome: { outcome: "cancelled" } });
});
it("aborts superseded and disposed detail reads, even if the service ignores cancellation", async () => {
  const t = setup();
  t.taskDetails.mockImplementation(() => new Promise(() => {}));
  const old = t.c.taskDetails("old");
  const firstSignal = t.taskDetails.mock.calls[0][1];
  const next = t.c.taskDetails("new");
  try {
    expect(firstSignal?.aborted).toBe(true);
    t.c.dispose();
    expect(t.taskDetails.mock.calls[1][1]?.aborted).toBe(true);
    await Promise.all([old, next]);
    expect(t.c.state.taskLoading).toBe(false);
  } finally {
    t.c.dispose();
  }
});
it("bounds connection and detail waits and closes a late connection", async () => {
  vi.useFakeTimers();
  const t = setup();
  let finish!: (value: { runtime: RuntimeClient; mode: "s1" }) => void;
  const connect = vi.fn(
    (_options: ClientOptions, _signal: AbortSignal) =>
      new Promise<{ runtime: RuntimeClient; mode: "s1" }>((resolve) => {
        finish = resolve;
      }),
  );
  const details = vi.fn(() => new Promise<ExecutionTaskDetails>(() => {}));
  const c = createAssistant({ connect, taskDetails: details }, () => "id");
  try {
    const connecting = c.connect(),
      reading = c.taskDetails("r");
    await vi.advanceTimersByTimeAsync(15_001);
    expect(c.state.connection).toBe("disconnected");
    expect(c.state.taskLoading).toBe(false);
    expect(connect.mock.calls[0][1].aborted).toBe(true);
    await Promise.all([connecting, reading]);
    finish({ runtime: t.client, mode: "s1" });
    await Promise.resolve();
    expect(t.client.close).toHaveBeenCalledOnce();
    expect(t.client.initialize).not.toHaveBeenCalled();
  } finally {
    c.dispose();
    t.c.dispose();
    vi.useRealTimers();
  }
});
it("updates both attention and background entries when time expires, without new events", async () => {
  vi.useFakeTimers();
  vi.setSystemTime(100);
  const t = setup(Date.now);
  try {
    await t.c.connect();
    t.view.capabilities.structuredQuestion = "supported";
    t.view.interactions.q = {
      commandId: "p",
      generation: t.view.generation,
      status: "pending",
      expiresAtMs: 1000,
      callbackLifetime: "generation_bound",
      request: {},
    };
    t.emit();
    expect(t.c.background.value).toEqual(["session-1"]);
    expect(t.c.attention.value).toBe(1);
    await vi.advanceTimersByTimeAsync(1001);
    expect(t.c.background.value).toEqual([]);
    expect(t.c.attention.value).toBe(0);
    t.view.interactions.q.expiresAtMs = 5000;
    t.view.interactions.q.generation = "old";
    t.emit();
    expect(t.c.attention.value).toBe(0);
  } finally {
    t.c.dispose();
    expect(vi.getTimerCount()).toBe(0);
    vi.useRealTimers();
  }
});

it("aborts reconnects and closes acquired clients when initialization never settles", async () => {
  vi.useFakeTimers();
  const t = setup();
  const connect = vi.fn(
    async (_options: ClientOptions, _signal: AbortSignal) => ({
      runtime: t.client,
      mode: "s1" as const,
    }),
  );
  vi.mocked(t.client.initialize).mockImplementation(
    () => new Promise(() => {}),
  );
  const c = createAssistant({ connect }, () => "id");
  try {
    const old = c.connect();
    await Promise.resolve();
    const current = c.connect();
    expect(connect.mock.calls[0][1].aborted).toBe(true);
    await old;
    expect(t.client.close).toHaveBeenCalledOnce();
    await vi.advanceTimersByTimeAsync(15_001);
    await current;
    expect(connect.mock.calls[1][1].aborted).toBe(true);
    expect(c.state.connection).toBe("disconnected");
    expect(t.client.close).toHaveBeenCalledTimes(2);
  } finally {
    c.dispose();
    t.c.dispose();
    expect(vi.getTimerCount()).toBe(0);
    vi.useRealTimers();
  }
});
it("aborts an in-flight connection on disposal and suppresses its late permissions", async () => {
  let options!: ClientOptions;
  const connect = vi.fn(async (input: ClientOptions, _signal: AbortSignal) => {
    options = input;
    return new Promise<{ runtime: RuntimeClient; mode: "s1" }>(() => {});
  });
  const c = createAssistant({ connect }, () => "id");
  const pending = c.connect();
  c.dispose();
  await pending;
  expect(connect.mock.calls[0][1].aborted).toBe(true);
  expect(
    await options.requestPermission!(
      {
        sessionId: "s",
        toolCall: { toolCallId: "t", title: "t" },
        options: [{ optionId: "once", name: "Allow", kind: "allow_once" }],
      },
      new AbortController().signal,
    ),
  ).toEqual({ outcome: { outcome: "cancelled" } });
  expect(c.state.permissions.size).toBe(0);
});

it("connection panel sends exactly the confirmed preview once and never includes unconfirmed history", async () => {
  const t = setup();
  const preview = {
    schemaVersion: 5 as const,
    kind: "historyPreview" as const,
    sessionId: "session-1",
    connectionId: "config-1",
    configRevision: 1,

    throughSequence: 4,
    commandIds: ["previous"],
    messageIds: ["answer"],
    text: "User:\nprevious\n\nAssistant:\nanswer",
    contentHash: "a".repeat(64),
  };
  const previewHistory = vi.fn().mockResolvedValue(preview);
  Object.assign(t.client, { previewHistory });
  await t.c.connect();
  await t.c.select("session-1");
  t.view.selectedConnectionId = "config-1";
  t.emit();
  const wrapper = mount(Connections, { props: { controller: t.c } });
  try {
    await flushPromises();
    const history = wrapper
      .findAll("label")
      .find((label) => label.text().startsWith("带入历史"))!
      .get("select");
    t.c.draft.value = "default input";
    await t.c.prompt();
    expect(t.submit.mock.calls.at(-1)![0].input.history).toBeUndefined();
    await history.setValue("recent");
    await flushPromises();
    expect(previewHistory).toHaveBeenLastCalledWith("session-1", "config-1", 5);
    expect(wrapper.get(".history-preview pre").text()).toBe(preview.text);
    t.c.draft.value = "unconfirmed input";
    await t.c.prompt();
    expect(t.submit.mock.calls.at(-1)![0].input.history).toBeUndefined();
    await wrapper.get(".history-preview button").trigger("click");
    t.c.draft.value = "confirmed input";
    await t.c.prompt();
    expect(t.submit.mock.calls.at(-1)![0].input.history).toEqual(preview);
    t.c.draft.value = "following input";
    await t.c.prompt();
    expect(t.submit.mock.calls.at(-1)![0].input.history).toBeUndefined();
    await history.setValue("all");
    await flushPromises();
    expect(previewHistory).toHaveBeenLastCalledWith(
      "session-1",
      "config-1",
      undefined,
    );
    await wrapper.get(".history-preview button").trigger("click");
    await history.setValue("none");
    t.c.draft.value = "cleared confirmation";
    await t.c.prompt();
    expect(t.submit.mock.calls.at(-1)![0].input.history).toBeUndefined();
  } finally {
    wrapper.unmount();
    await t.c.dispose();
  }
});

it("drops a history preview that completes after the selected session changed", async () => {
  const t = setup();
  let finish!: (value: unknown) => void;
  const previewHistory = vi.fn(
    () => new Promise((resolve) => (finish = resolve)),
  );
  Object.assign(t.client, { previewHistory });
  await t.c.connect();
  await t.c.select("session-1");
  t.view.selectedConnectionId = "config-1";
  t.emit();
  const wrapper = mount(Connections, { props: { controller: t.c } });
  await flushPromises();
  const history = wrapper
    .findAll("label")
    .find((label) => label.text().startsWith("带入历史"))!
    .get("select");
  await history.setValue("all");
  await flushPromises();
  t.c.state.selected = "session-2";
  await wrapper.vm.$nextTick();
  finish({
    schemaVersion: 5,
    kind: "historyPreview",
    sessionId: "session-1",
    connectionId: "config-1",
    configRevision: 1,
    throughSequence: 1,
    commandIds: [],
    messageIds: [],
    text: "stale preview",
    contentHash: "a".repeat(64),
  });
  await flushPromises();
  expect(wrapper.text()).not.toContain("stale preview");
  wrapper.unmount();
  t.c.dispose();
});

it("maps history capacity and user cancellation without a generic retry error", () => {
  expect(operationMessage("limit_exceeded")).toContain("64 KiB");
  expect(operationMessage("cancelled")).toBe("");
});

it("deleting the selected connection preserves history and immediately disables ordinary input", async () => {
  const t = setup();
  await t.c.connect();
  await t.c.select("session-1");
  t.c.draft.value = "keep draft";
  const before = t.c.view.value;
  const saveConnection = vi.fn().mockResolvedValue({});
  Object.assign(t.client, { saveConnection });
  const wrapper = mount(Connections, {
    props: { controller: t.c },
    attachTo: document.body,
  });
  try {
    await flushPromises();
    expect(t.c.canSend.value).toBe(true);
    const deleteButton = wrapper.get('button[aria-label="删除连接 Fixture"]');
    expect(wrapper.get('button[aria-label="编辑连接 Fixture"]')).toBeTruthy();
    expect(
      wrapper.get('button[aria-label="将连接 Fixture 设为默认"]'),
    ).toBeTruthy();
    vi.mocked(t.client.connections).mockResolvedValue({
      schemaVersion: 5,
      kind: "connectionPage",
      preferences: { schemaVersion: 5, kind: "userPreferences" },
      connections: [],
    });
    await deleteButton.trigger("click");
    await flushPromises();
    expect(saveConnection).not.toHaveBeenCalled();
    const dialog = wrapper.get('[role="alertdialog"]');
    expect(dialog.attributes("aria-modal")).toBe("true");
    expect(dialog.text()).toContain("Fixture");
    expect(document.activeElement?.textContent).toContain("取消删除");
    await wrapper
      .findAll("button")
      .find((b) => b.text() === "取消删除")!
      .trigger("click");
    await flushPromises();
    expect(document.activeElement).toBe(deleteButton.element);
    expect(saveConnection).not.toHaveBeenCalled();
    await wrapper
      .findAll("button")
      .find((b) => b.text() === "删除")!
      .trigger("click");
    await wrapper
      .findAll("button")
      .find((b) => b.text() === "确认删除")!
      .trigger("click");
    await flushPromises();
    expect(saveConnection.mock.calls[0][0].status).toBe("deleted");
    expect(t.c.view.value).toEqual(before);
    expect(t.c.canSend.value).toBe(false);
    expect(wrapper.text()).toContain("当前会话需要选择可用连接");
    await t.c.prompt();
    expect(t.submit).not.toHaveBeenCalled();
  } finally {
    wrapper.unmount();
    t.c.dispose();
  }
});

it("connection revision conflicts invalidate stale edit and delete actions", async () => {
  const t = setup();
  await t.c.connect();
  const saveConnection = vi
    .fn()
    .mockRejectedValue(new ClientError("revision_conflict"));
  Object.assign(t.client, { saveConnection });
  const wrapper = mount(Connections, { props: { controller: t.c } });
  try {
    await flushPromises();
    await wrapper.get('button[aria-label="编辑连接 Fixture"]').trigger("click");
    await wrapper.get("form").trigger("submit");
    await flushPromises();
    expect(wrapper.get("form h3").text()).toBe("添加连接");
    expect(wrapper.text()).toContain("目录已刷新，请重新打开连接");
    await wrapper.get('button[aria-label="删除连接 Fixture"]').trigger("click");
    await wrapper
      .findAll("button")
      .find((button) => button.text() === "确认删除")!
      .trigger("click");
    await flushPromises();
    expect(wrapper.find('[role="alertdialog"]').exists()).toBe(false);
    expect(saveConnection).toHaveBeenCalledTimes(2);
  } finally {
    wrapper.unmount();
    t.c.dispose();
  }
});

it("existing configuration cannot become a custom API key and Claude supports configuration reuse", async () => {
  const t = setup();
  await t.c.connect();
  const wrapper = mount(Connections, { props: { controller: t.c } });
  try {
    await flushPromises();
    await wrapper
      .findAll("button")
      .find((b) => b.text() === "编辑")!
      .trigger("click");
    const field = (name: string) =>
      wrapper
        .findAll("label")
        .find((l) => l.text().startsWith(name))!
        .get("select");
    await field("认证来源").setValue("custom_api");
    expect(wrapper.text()).not.toContain("已选择凭据");
    expect(
      wrapper
        .findAll("button")
        .find((b) => b.text() === "验证并保存")!
        .attributes("disabled"),
    ).toBeDefined();
    await field("认证来源").setValue("existing_config");
    await field("服务").setValue("claude");
    expect(field("认证来源").element.value).toBe("existing_config");
    expect(
      field("认证来源")
        .findAll("option")
        .some((o) => o.attributes("value") === "existing_config"),
    ).toBe(true);
  } finally {
    wrapper.unmount();
    t.c.dispose();
  }
});
