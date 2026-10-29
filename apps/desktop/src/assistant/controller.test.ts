const flushPromises = () =>
  new Promise<void>((resolve) => setTimeout(resolve, 0));
import { taskFromFacts } from "../../../../tests/self-service/support";

import { activeStage } from "@rss-mdm-agent/ai-contract";
import { deepClone } from "valtio/vanilla/utils";
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
import type {
  TaskDetail,
  ExecutionTaskDetails,
} from "@rss-mdm-agent/execution-bindings/tasks";
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
      deliveries: {},
    };
  view.connection = "attached";
  const submit = vi.fn().mockResolvedValue({ kind: "receipt" });
  const client = {
    initialize: vi.fn().mockResolvedValue({ contractVersion: 1, acp: 1 }),
    connections: vi.fn().mockResolvedValue({
      preferences: {
        schemaVersion: 1,
        kind: "userPreferences",
        defaultConnectionId: "config-1",
      },
      connections: [
        {
          schemaVersion: 1,
          kind: "connection",
          connectionId: "config-1",
          presentation: { name: "Fixture" },
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
      .mockResolvedValue({ schemaVersion: 1, kind: "userPreferences" }),
    listSessions: vi.fn().mockResolvedValue({
      items: [
        {
          namespace: session.namespace,
          status: session.status,
          presentation: { title: "已有对话" },
          lastActivityAtMs: 0,
        },
      ],
    }),
    restore: vi.fn().mockResolvedValue(view),
    createSession: vi
      .fn()
      .mockImplementation(async ({ sessionId }) => ({ sessionId })),
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
  it("orders selection saves and ignores a stale failure after a new selection or connection", async () => {
    const t = setup();
    await t.c.connect();
    let reject!: (error: Error) => void;
    vi.mocked(t.client.savePreferences).mockImplementationOnce(
      () =>
        new Promise((_, fail) => {
          reject = fail;
        }),
    );
    await t.c.select("session-1");
    await flushPromises();
    t.c.state.sessions.set("session-2", {
      ...t.c.state.sessions.get("session-1")!,
      namespace: { ...t.view.namespace, sessionId: "session-2" },
    });
    await t.c.select("session-2");
    await flushPromises();
    expect(t.client.savePreferences).toHaveBeenCalledTimes(1);
    reject(new Error("old save failed"));
    await flushPromises();
    expect(t.client.savePreferences).toHaveBeenLastCalledWith({
      selectedSessionId: { set: "session-2" },
    });
    expect(t.c.state.preferenceSave).toBeUndefined();
    vi.mocked(t.client.savePreferences).mockImplementationOnce(
      () =>
        new Promise((_, fail) => {
          reject = fail;
        }),
    );
    await t.c.select("session-1");
    await flushPromises();
    await t.c.connect();
    await t.c.retryPreference();
    expect(t.c.state.preferenceSave).toBeUndefined();
    reject(new Error("previous client failed"));
    await flushPromises();
    expect(t.c.state.preferenceSave).toBeUndefined();
    expect(t.submit).not.toHaveBeenCalled();
    t.c.dispose();
  });
  it("queues through Host while provider capabilities independently gate steer, cancel and resume", async () => {
    const t = setup();
    await t.c.connect();
    await t.c.select("session-1");
    t.view.commands.p = {
      command: {
        schemaVersion: 1,
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
    t.c.draft.set("editable while busy");
    expect(t.c.busy()).toBe(true);
    expect(t.c.canSend()).toBe(true);
    expect(t.c.canSteer()).toBe(false);
    expect(t.c.canResume()).toBe(false);
    await t.c.prompt();
    expect(t.submit.mock.calls[0][0].input.policy).toBe("queue_next");
    t.submit.mockClear();
    t.c.draft.set("editable while busy");
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
    expect(t.c.canSteer()).toBe(false);
    expect(t.c.canCancel()).toBe(false);
    expect(t.c.canResume()).toBe(true);
    for (const settled of ["acknowledged", "cancelled"] as const) {
      t.view.commands.p.state = settled;
      t.emit();
      expect(t.c.busy()).toBe(false);
      expect(t.c.canSteer()).toBe(false);
      expect(t.c.canCancel()).toBe(false);
    }
    t.c.dispose();
  });
  it("does not turn receipts into history and retries exactly the unknown command without erasing a newer draft", async () => {
    const t = setup();
    await t.c.connect();
    await t.c.select("session-1");
    t.submit.mockRejectedValueOnce(new ClientError("request_failed"));
    t.c.draft.set("original");
    await t.c.prompt();
    const original = JSON.stringify(t.submit.mock.calls[0][0]);
    expect(t.c.view()?.timeline).toEqual([]);
    t.c.draft.set("newer edit");
    await t.c.retry();
    expect(JSON.stringify(t.submit.mock.calls[1][0])).toBe(original);
    expect(t.c.draft.get()).toBe("newer edit");
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
  it("retains same-user display but invalidates transport-bound callbacks on reconnect", async () => {
    const t = setup();
    await t.c.connect();
    await t.c.select("session-1");
    t.c.draft.set("private caller draft");
    const result = t.options().requestPermission!(
      {
        sessionId: "session-1",
        toolCall: { toolCallId: "call", title: "tool" },
        options: [],
      },
      new AbortController().signal,
    );
    await t.c.connect();
    expect(t.c.state.views.size).toBe(1);
    expect(t.c.draft.get()).not.toBe("");
    expect(await result).toEqual({ outcome: { outcome: "cancelled" } });
    t.c.dispose();
  });
  it("retains only the latest authorized execution read and clears it on a failed read", async () => {
    const t = setup();
    let release: (v: TaskDetail) => void = () => {};
    t.taskDetails
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            release = resolve;
          }),
      )
      .mockResolvedValueOnce(
        taskFromFacts({
          kind: "execution",
          value: fixtures.outcomeUnknown as ExecutionTaskDetails,
        }),
      )
      .mockRejectedValueOnce(new Error("secret server text"));
    const old = t.c.taskDetails("old");
    await t.c.taskDetails("current");
    release(
      taskFromFacts({
        kind: "execution",
        value: fixtures.running as ExecutionTaskDetails,
      }),
    );
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
    t.c.create();
    t.c.draft.set("first");
    await t.c.prompt();
    expect(t.c.state.listError).toBe("unavailable");
    expect(t.c.state.createError).toBe("invalid_input");
    await t.c.list(false);
    expect(t.c.state.listError).toBe("");
    expect(t.c.state.createError).toBe("invalid_input");
    vi.mocked(t.client.restore).mockImplementationOnce(async (id) => ({
      ...structuredClone(t.view),
      namespace: { ...t.view.namespace, sessionId: id },
    }));
    await t.c.prompt();
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
    t.c.draft.set("private");
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
    expect(t.c.state.views.size).toBe(1);
    expect(t.c.draft.get()).not.toBe("");
    expect(t.c.state.cleanupError).toBe("cleanup_failed");
    expect(() => t.c.dispose()).not.toThrow();
    expect(t.c.runtime.current).toBeUndefined();
    expect(t.c.state.connection).toBe("disconnected");
  });
  it("distinguishes detach, active runs, retired sessions and unavailable continuation", async () => {
    const t = setup();
    await t.c.connect();
    await t.c.select("session-1");
    t.view.capabilities.continuation = "unknown";
    t.emit();
    expect(t.c.resumeReason()).toBe("unknown");
    t.view.capabilities.continuation = "unsupported";
    t.emit();
    expect(t.c.resumeReason()).toBe("unsupported");
    t.view.capabilities.continuation = "across_processes";
    t.view.connection = "detached";
    t.emit();
    expect(t.c.sessionConnection()).toBe("detached");
    expect(t.c.resumeReason()).toBe("not-attached");
    t.view.connection = "attached";
    t.view.sessionStatus = "retired";
    t.emit();
    expect(t.c.resumeReason()).toBe("retired");
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
  const details = vi.fn(() => new Promise<TaskDetail>(() => {}));
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
    expect(t.c.background()).toEqual(["session-1"]);
    expect(t.c.attention()).toBe(1);
    await vi.advanceTimersByTimeAsync(1001);
    expect(t.c.background()).toEqual([]);
    expect(t.c.attention()).toBe(0);
    t.view.interactions.q.expiresAtMs = 5000;
    t.view.interactions.q.generation = "old";
    t.emit();
    expect(t.c.attention()).toBe(0);
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

it("maps history capacity and user cancellation without a generic retry error", () => {
  expect(operationMessage("limit_exceeded")).toContain("64 KiB");
  expect(operationMessage("cancelled")).toBe("");
  expect(operationMessage("verification_cancelled")).toContain(
    "已保存配置保留",
  );
  expect(operationMessage("verification_refused")).toContain("已保存配置保留");
});

it("retains same-user history, draft and unknown command through a failed reconnect without sending", async () => {
  const t = setup();
  await t.c.connect();
  await t.c.select("session-1");
  t.c.draft.set("unsent private draft");
  t.submit.mockRejectedValueOnce(new ClientError("request_failed"));
  await t.c.prompt();
  const pending = t.c.state.pending.get("session-1");
  expect(pending).toBeDefined();
  t.c.draft.set("new draft");
  vi.mocked(t.client.initialize).mockRejectedValueOnce(
    new ClientError("transport_closed"),
  );
  await t.c.connect();
  expect(t.c.state.connection).toBe("disconnected");
  expect(t.c.state.views.has("session-1")).toBe(true);
  expect(t.c.state.selected).toBe("session-1");
  expect(t.c.draft.get()).toBe("new draft");
  expect(t.c.state.pending.get("session-1")).toEqual(pending);
  expect(t.c.canSend()).toBe(false);
  expect(t.submit).toHaveBeenCalledTimes(1);
  t.c.dispose();
  expect(t.c.state.views.size).toBe(0);
  expect(t.c.state.pending.size).toBe(0);
  expect(t.c.state.drafts.size).toBe(0);
});

it("a single failed history restore leaves the healthy connection usable without resending", async () => {
  const t = setup();
  await t.c.connect();
  await t.c.select("session-1");
  t.c.draft.set("keep");
  vi.mocked(t.client.restore).mockRejectedValueOnce(
    new ClientError("context_unavailable"),
  );
  await t.c.connect();
  expect(t.c.state.connection).toBe("connected");
  expect(t.c.runtime.current).toBeDefined();
  expect(t.c.state.errors.get("session-1")).toBe("context_unavailable");
  expect(t.c.state.views.get("session-1")?.connection).toBe("detached");
  expect(t.c.draft.get()).toBe("keep");
  expect(t.c.canSend()).toBe(false);
  expect(t.submit).not.toHaveBeenCalled();
  await t.c.refreshConnections();
  expect(t.c.state.connections).toHaveLength(1);
  t.c.create();
  expect(t.c.state.selected).toBe("");
  expect(t.client.createSession).not.toHaveBeenCalled();
  t.c.dispose();
});

it("a transport lost during the selected history restore is not promoted back to connected", async () => {
  const t = setup();
  let disconnect!: () => void;
  Object.assign(t.client.connection, {
    closed: new Promise<void>((resolve) => (disconnect = resolve)),
  });
  await t.c.connect();
  await t.c.select("session-1");
  vi.mocked(t.client.restore).mockImplementationOnce(async () => {
    disconnect();
    throw new ClientError("transport_closed");
  });
  await t.c.connect();
  expect(t.c.state.connection).toBe("disconnected");
  expect(t.c.canSend()).toBe(false);
  expect(t.submit).not.toHaveBeenCalled();
  t.c.dispose();
});

it("first send is single-flight and unknown creation retries the same identity without losing a newer draft", async () => {
  const t = setup();
  await t.c.connect();
  t.c.draft.set("first message");
  let finish!: (value: { sessionId: string }) => void;
  vi.mocked(t.client.createSession).mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  vi.mocked(t.client.restore).mockImplementation(async (id) => ({
    ...structuredClone(t.view),
    namespace: { ...t.view.namespace, sessionId: id },
  }));
  const first = t.c.prompt(),
    duplicate = t.c.prompt();
  await flushPromises();
  expect(t.client.createSession).toHaveBeenCalledTimes(1);
  const intent = vi.mocked(t.client.createSession).mock.calls[0][0];
  finish({ sessionId: intent.sessionId });
  await Promise.all([first, duplicate]);
  expect(t.submit).toHaveBeenCalledTimes(1);
  expect(t.submit.mock.calls[0][0].sessionId).toBe(intent.sessionId);
  expect(t.c.draft.get()).toBe("");
  t.c.create();
  t.c.draft.set("retry me");
  vi.mocked(t.client.createSession).mockRejectedValueOnce(
    new ClientError("transport_closed"),
  );
  await t.c.prompt();
  expect(t.c.draft.get()).toBe("retry me");
  await t.c.prompt();
  expect(vi.mocked(t.client.createSession).mock.calls[1][0]).toEqual(
    vi.mocked(t.client.createSession).mock.calls[2][0],
  );
  expect(t.submit).toHaveBeenCalledTimes(2);
  t.c.dispose();
});

it("first creation finishing after navigation never changes selection or sends in the background", async () => {
  const t = setup();
  await t.c.connect();
  t.c.draft.set("blank draft");
  let finish!: (value: { sessionId: string }) => void;
  vi.mocked(t.client.createSession).mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  vi.mocked(t.client.restore).mockImplementation(async (id) => ({
    ...structuredClone(t.view),
    namespace: { ...t.view.namespace, sessionId: id },
  }));
  const sending = t.c.prompt();
  await flushPromises();
  await t.c.select("session-1");
  finish({
    sessionId: vi.mocked(t.client.createSession).mock.calls[0][0].sessionId,
  });
  await sending;
  expect(t.c.state.selected).toBe("session-1");
  expect(t.submit).not.toHaveBeenCalled();
  expect(t.c.state.drafts.get("")?.text).toBe("blank draft");
  t.c.dispose();
});

it.each(["pending", "finished"])(
  "explicit new conversation discards the %s previous first-send intent",
  async (completion) => {
    const t = setup();
    await t.c.connect();
    t.c.draft.set("old draft");
    let finish!: (value: { sessionId: string }) => void;
    vi.mocked(t.client.createSession).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    vi.mocked(t.client.restore).mockImplementation(async (id) => ({
      ...structuredClone(t.view),
      namespace: { ...t.view.namespace, sessionId: id },
    }));
    const old = t.c.prompt();
    await flushPromises();
    const previous = vi.mocked(t.client.createSession).mock.calls[0][0];
    await t.c.select("session-1");
    if (completion === "finished") {
      finish({ sessionId: previous.sessionId });
      await old;
    }
    t.c.create();
    t.c.state.connections.push({
      ...t.c.state.connections[0],
      connectionId: "config-2",
    });
    t.c.state.blankConnectionId = "config-2";
    t.c.draft.set("new conversation");
    await t.c.prompt();
    if (completion === "pending") {
      finish({ sessionId: previous.sessionId });
      await old;
    }
    expect(t.client.createSession).toHaveBeenCalledTimes(2);
    const fresh = vi.mocked(t.client.createSession).mock.calls[1][0];
    expect(fresh.sessionId).not.toBe(previous.sessionId);
    expect(fresh.connectionId).toBe("config-2");
    expect(t.submit).toHaveBeenCalledTimes(1);
    expect(t.submit.mock.calls[0][0].sessionId).toBe(fresh.sessionId);
    expect(t.submit.mock.calls[0][0].input.text).toBe("new conversation");
    expect(t.c.state.selected).toBe(fresh.sessionId);
    expect(t.c.state.opening).toBe(false);
    t.c.dispose();
  },
);

it("restores a known first session after attachment failure without creating again", async () => {
  const t = setup();
  await t.c.connect();
  t.c.draft.set("first");
  vi.mocked(t.client.restore).mockRejectedValueOnce(
    new ClientError("transport_closed"),
  );
  await t.c.prompt();
  const id = vi.mocked(t.client.createSession).mock.calls[0][0].sessionId;
  expect(t.c.draft.get()).toBe("first");
  expect(t.submit).not.toHaveBeenCalled();
  vi.mocked(t.client.restore).mockResolvedValue({
    ...t.view,
    namespace: { ...t.view.namespace, sessionId: id },
  });
  await t.c.prompt();
  expect(t.client.createSession).toHaveBeenCalledTimes(1);
  expect(t.submit).toHaveBeenCalledTimes(1);
  t.c.dispose();
});

it("missing configuration and edits during creation preserve the blank draft", async () => {
  const t = setup();
  await t.c.connect();
  t.c.state.connections = [];
  t.c.draft.set("keep");
  await t.c.prompt();
  expect(t.c.state.createError).toBe("connection_required");
  expect(t.client.createSession).not.toHaveBeenCalled();
  expect(t.c.draft.get()).toBe("keep");
  await t.c.refreshConnections();
  let finish!: (value: { sessionId: string }) => void;
  vi.mocked(t.client.createSession).mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  vi.mocked(t.client.restore).mockImplementation(async (id) => ({
    ...t.view,
    namespace: { ...t.view.namespace, sessionId: id },
  }));
  const sending = t.c.prompt();
  await flushPromises();
  t.c.draft.set("edited");
  finish({
    sessionId: vi.mocked(t.client.createSession).mock.calls[0][0].sessionId,
  });
  await sending;
  expect(t.c.draft.get()).toBe("edited");
  expect(t.c.state.selected).toBe("");
  expect(t.submit).not.toHaveBeenCalled();
  await t.c.prompt();
  expect(t.client.createSession).toHaveBeenCalledTimes(1);
  expect(t.submit.mock.calls[0][0].input.text).toBe("edited");
  t.c.dispose();
});

it("execution cards require a Host delivery and matching Rust conversation and tool identity", async () => {
  const t = setup();
  await t.c.connect();
  await t.c.select("session-1");
  const signal = new AbortController().signal;
  await expect(t.c.executionDetails("op", signal)).rejects.toThrow();
  expect(t.taskDetails).not.toHaveBeenCalled();
  const details = structuredClone(fixtures.running) as ExecutionTaskDetails;
  t.view.deliveries.op = {
    commandId: "command",
    recorded: true,
    proposal: {
      name: "execution_execute",
      arguments: {
        request: details.status.operationRequestId,
        task: "backend-task",
        attempt: "backend-attempt",
        revision: "a".repeat(64),
      },
    },
  };
  t.emit();
  t.taskDetails.mockResolvedValue(
    taskFromFacts({ kind: "execution", value: details }),
  );
  await expect(t.c.executionDetails("op", signal)).rejects.toThrow();
  details.action.initiator = {
    kind: "ai",
    conversation: "session-1",
    toolCall: "op",
    provider: "codex",
    config: { id: "config", revision: "1" },
    osSession: {
      ...(details.action.initiator as { osSession: object }).osSession,
    },
  } as Extract<ExecutionTaskDetails["action"]["initiator"], { kind: "ai" }>;
  await expect(t.c.executionDetails("op", signal)).resolves.toEqual(
    taskFromFacts({ kind: "execution", value: details }),
  );
  // An ambiguous union is not an execution request, even if one field has an ID.
  t.view.deliveries.op.proposal.arguments.script = {
    operationRequestId: "other",
  };
  t.emit();
  t.taskDetails.mockClear();
  await expect(t.c.executionDetails("op", signal)).rejects.toThrow();
  expect(t.taskDetails).not.toHaveBeenCalled();
  delete t.view.deliveries.op.proposal.arguments.script;
  t.emit();
  details.action.initiator.toolCall = "forged";
  await expect(t.c.executionDetails("op", signal)).rejects.toThrow();
  t.c.dispose();
});

it("later-page Policy cards read the original native request and keep conversation/tool binding", async () => {
  const t = setup();
  await t.c.connect();
  await t.c.select("session-1");
  const signal = new AbortController().signal;
  const details = structuredClone(fixtures.running) as ExecutionTaskDetails;
  details.status.operationRequestId = "op";
  details.action.initiator = {
    kind: "ai",
    conversation: "session-1",
    toolCall: "op",
    provider: "codex",
    config: { id: "config", revision: "1" },
    osSession: {
      ...(details.action.initiator as { osSession: object }).osSession,
    },
  } as Extract<ExecutionTaskDetails["action"]["initiator"], { kind: "ai" }>;
  t.view.deliveries.op = {
    commandId: "command",
    recorded: true,
    proposal: {
      name: "execution_policy_invoke",
      arguments: {
        tool: "policy_later",

        parameters: { request: "model-suggested-id" },
      },
    },
  };
  t.emit();
  t.taskDetails.mockResolvedValue(
    taskFromFacts({ kind: "execution", value: details }),
  );
  await expect(t.c.executionDetails("op", signal)).resolves.toEqual(
    taskFromFacts({ kind: "execution", value: details }),
  );
  expect(t.taskDetails).toHaveBeenLastCalledWith("op", expect.any(AbortSignal));
  details.status.operationRequestId = "model-suggested-id";
  await expect(t.c.executionDetails("op", signal)).rejects.toThrow();
  details.status.operationRequestId = "op";
  details.action.initiator.toolCall = "other-tool";
  await expect(t.c.executionDetails("op", signal)).rejects.toThrow();
  details.action.initiator.toolCall = "op";
  details.action.initiator.conversation = "other-conversation";
  await expect(t.c.executionDetails("op", signal)).rejects.toThrow();
  t.c.dispose();
});

describe("resource composition ownership", () => {
  it("stages without sending and submits the preview to the existing session", async () => {
    const t = setup();
    await t.c.connect();
    await t.c.select("session-1");
    t.c.draft.set("解释版本");
    const context = samplePageContext();
    t.c.stageContext(context);
    expect(t.submit).not.toHaveBeenCalled();
    expect(t.client.createSession).not.toHaveBeenCalled();
    expect(t.c.context()?.text).toBe(context.text);
    await t.c.prompt();
    expect(t.submit.mock.calls[0][0].input.text).toBe(
      "解释版本\n\n" + context.text,
    );
    expect(t.submit.mock.calls[0][0].sessionId).toBe("session-1");
    expect(t.c.context()).toBeUndefined();
    expect(t.c.draft.get()).toBe("");
    t.c.dispose();
  });
  it("freezes unknown acceptance and does not clear subsequent edits or replacement", async () => {
    const t = setup();
    await t.c.connect();
    await t.c.select("session-1");
    t.c.draft.set("original");
    t.c.stageContext(samplePageContext());
    t.submit.mockRejectedValueOnce(new ClientError("transport_closed"));
    await t.c.prompt();
    const original = JSON.parse(JSON.stringify(t.submit.mock.calls[0][0]));
    t.c.draft.set("next question");
    t.c.stageContext(samplePageContext("另一个页面对象"));
    const next = t.c.context();
    await t.c.retry();
    expect(t.submit.mock.calls[1][0]).toEqual(original);
    expect(t.c.draft.get()).toBe("next question");
    expect(t.c.context()).toEqual(next);
    t.c.dispose();
    expect(t.c.state.drafts.size).toBe(0);
  });
  it("moves blank composition with first creation and refuses a changed attachment during creation", async () => {
    const t = setup();
    await t.c.connect();
    t.c.create();
    t.c.draft.set("question");
    t.c.stageContext(samplePageContext());
    vi.mocked(t.client.restore).mockImplementation(async (id) => ({
      ...structuredClone(t.view),
      namespace: { ...t.view.namespace, sessionId: id },
    }));
    let finish!: (v: { sessionId: string }) => void;
    vi.mocked(t.client.createSession).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    const first = t.c.prompt();
    await flushPromises();
    t.c.removeContext();
    finish({
      sessionId: vi.mocked(t.client.createSession).mock.calls[0][0].sessionId,
    });
    await first;
    expect(t.submit).not.toHaveBeenCalled();
    expect(t.c.draft.get()).toBe("question");
    await t.c.prompt();
    expect(t.submit.mock.calls[0][0].input.text).toBe("question");
    expect(t.client.createSession).toHaveBeenCalledTimes(1);
    t.c.dispose();
  });
  it("isolates composition by session, invalidates stale context and permits removal", async () => {
    const t = setup();
    await t.c.connect();
    await t.c.select("session-1");
    t.c.draft.set("question");
    t.c.stageContext(samplePageContext());
    t.c.create();
    expect(t.c.context()).toBeUndefined();
    await t.c.select("session-1");
    t.c.validatePageContexts([]);
    expect(t.c.context()?.stale).toBe(true);
    await t.c.prompt();
    expect(t.submit).not.toHaveBeenCalled();
    t.c.removeContext();
    await t.c.prompt();
    expect(t.submit.mock.calls[0][0].input.text).toBe("question");
    t.c.dispose();
  });
});

it("invalidates page attachments in every conversation while the sidebar is closed and retains question text", () => {
  const c = createAssistant(
    undefined,
    () => "id",
    () => 100,
    "tenant-a",
  );
  const make = (revision: string) =>
    pageContext(
      "software-management",
      "软件管理",
      [],
      [
        {
          id: "r",
          title: "Visible",
          description: "Only display",
          source: "service",
          revision,
          validUntil: 1000,
        },
      ],
      "tenant-a",
      100,
    );
  const original = make("one");
  c.stageContext(original);
  c.draft.set("keep my question");
  c.state.drafts.set("background", {
    text: "background question",
    context: original,
  });
  c.validatePageContexts([make("two")]);
  expect(c.context()?.stale).toBe(true);
  expect(c.draft.get()).toBe("keep my question");
  expect(c.state.drafts.get("background")?.context?.stale).toBe(true);
  c.dispose();
  expect(c.state.drafts.size).toBe(0);
});
import { pageContext } from "../workspace/page-context";

function samplePageContext(title = "办公套件") {
  return pageContext(
    "software-management",
    "软件管理",
    [],
    [
      {
        id: "published-policy",
        title,
        description: "已发布软件展示",
        source: "授权目录",
        revision: "v1",
      },
    ],
    "anonymous",
    100,
  );
}

it("keeps an unchanged page revision after a repeat observation without renewing its frozen deadline", () => {
  let now = 1000;
  const c = createAssistant(
    undefined,
    () => "id",
    () => now,
    "tenant-a",
  );
  const make = (collectedAt: number, validUntil: number) =>
    pageContext(
      "home",
      "主页",
      [],
      [
        {
          id: "task",
          title: "Task",
          description: "原任务阶段：running",
          source: "service",
          revision: "one",
          collectedAt,
          validUntil,
        },
      ],
      "tenant-a",
      now,
    );
  const original = make(1000, 2000);
  c.stageContext(original);
  c.draft.set("解释这个任务");
  now = 1500;
  const reread = make(1500, 3500);
  c.validatePageContexts([reread]);
  expect(c.context()?.stale).toBe(false);
  expect(c.context()?.text).toBe(original.text);
  expect(c.context()?.expiresAt).toBe(2000);
  now = 2000;
  c.validatePageContexts([reread]);
  expect(c.context()?.stale).toBe(true);
  expect(c.draft.get()).toBe("解释这个任务");
  c.dispose();
});

function historyPreview(
  sessionId = "session-1",
  connectionId = "config-1",
  throughSequence = 0,
) {
  return {
    schemaVersion: 1 as const,
    kind: "historyPreview" as const,
    sessionId,
    connectionId,
    configRevision: 1,
    throughSequence,
    commandIds: ["completed-command"],
    messageIds: ["completed-message"],
    text: "已完成的稳定原文",
    contentHash: "a".repeat(64),
  };
}
function alternative(t: ReturnType<typeof setup>) {
  t.c.state.connections.push({
    ...t.c.state.connections[0],
    connectionId: "config-2",
    presentation: { name: "Alternative" },
  });
}
it("chooses a ready connection for an empty conversation after deletion of the default, without creating or sending until the user submits", async () => {
  const t = setup();
  await t.c.connect();
  alternative(t);
  t.c.create();
  t.c.state.connections[0].status = "deleted";
  delete t.c.state.preferences.defaultConnectionId;
  expect(t.c.canSend()).toBe(false);
  expect(await t.c.chooseConnection("config-2")).toBe(true);
  expect(t.c.canSend()).toBe(true);
  expect(t.c.state.preferences.defaultConnectionId).toBeUndefined();
  expect(t.client.createSession).not.toHaveBeenCalled();
  expect(t.submit).not.toHaveBeenCalled();
  vi.mocked(t.client.restore).mockImplementation(async (id) => ({
    ...structuredClone(t.view),
    namespace: { ...t.view.namespace, sessionId: id },
    selectedConnectionId: "config-2",
  }));
  t.c.draft.set("明确发送");
  await t.c.prompt();
  expect(t.client.createSession).toHaveBeenCalledWith({
    sessionId: expect.any(String),
    connectionId: "config-2",
  });
  expect(t.submit).toHaveBeenCalledOnce();
  t.c.dispose();
});
it("keeps an uncertain connection change read-only until the original session is read, and ignores a late result after reconnect", async () => {
  const t = setup();
  await t.c.connect();
  await t.c.select("session-1");
  alternative(t);
  const choose = vi
    .fn()
    .mockRejectedValueOnce(new ClientError("transport_failed"));
  Object.assign(t.client, { selectConnection: choose });
  expect(await t.c.chooseConnection("config-2")).toBe(false);
  expect(t.c.state.optionPending.has("session-1")).toBe(true);
  expect(t.c.canSend()).toBe(false);
  await t.c.chooseConnection("config-2");
  expect(choose).toHaveBeenCalledOnce();
  await t.c.restore();
  expect(t.c.state.optionPending.size).toBe(0);
  let finish!: (view: SessionView) => void;
  choose.mockImplementationOnce(
    () =>
      new Promise<SessionView>((resolve) => {
        finish = resolve;
      }),
  );
  const pending = t.c.chooseConnection("config-2");
  expect(t.c.state.optionBusy).toBe(true);
  await t.c.connect();
  finish({ ...t.view, selectedConnectionId: "config-2" });
  await pending;
  expect(t.c.selectedConnectionId()).toBe("config-1");
  expect(t.c.state.optionBusy).toBe(false);
  t.c.dispose();
});
it("previews without carrying history, binds explicit confirmation to the exact payload and consumes it only once", async () => {
  const t = setup();
  await t.c.connect();
  await t.c.select("session-1");
  t.c.state.views.get("session-1")!.connectionPending = true;
  const preview = historyPreview();
  const read = vi.fn().mockResolvedValue(preview);
  Object.assign(t.client, { previewHistory: read });
  expect(await t.c.prepareHistory(2)).toBe(true);
  expect(read).toHaveBeenCalledWith("session-1", "config-1", 2);
  expect(t.c.state.history.has("session-1")).toBe(false);
  expect(t.submit).not.toHaveBeenCalled();
  t.c.draft.set("预览后未确认的输入");
  await t.c.prompt();
  expect(vi.mocked(t.client.submit).mock.calls[0][0].input).not.toHaveProperty(
    "history",
  );
  await t.c.prepareHistory();
  expect(t.c.confirmHistory()).toBe(true);
  t.c.draft.set("本人确认后的输入");
  await t.c.prompt();
  expect(vi.mocked(t.client.submit).mock.calls[1][0].input).toMatchObject({
    history: preview,
  });
  expect(t.c.state.history.size).toBe(0);
  t.c.draft.set("下一条");
  await t.c.prompt();
  expect(vi.mocked(t.client.submit).mock.calls[2][0].input).not.toHaveProperty(
    "history",
  );
  t.c.dispose();
});
it("fences history previews by selection, source watermark and connection revision and blocks stale confirmed history", async () => {
  const t = setup();
  await t.c.connect();
  await t.c.select("session-1");
  t.view.connectionPending = true;
  t.emit();
  const read = vi.fn().mockResolvedValue(historyPreview());
  Object.assign(t.client, { previewHistory: read });
  await t.c.prepareHistory(1);
  t.c.state.connections[0].configRevision = 2;
  expect(t.c.confirmHistory()).toBe(false);
  expect(t.c.state.history.size).toBe(0);
  t.c.state.connections[0].configRevision = 1;
  await t.c.prepareHistory(1);
  expect(t.c.confirmHistory()).toBe(true);
  t.emit({ ...t.view, cursor: 1 });
  t.c.draft.set("不能隐式丢弃历史并发送");
  expect(t.c.compositionError()).toContain("历史预览已变化");
  await t.c.prompt();
  expect(t.submit).not.toHaveBeenCalled();
  t.c.clearHistory();
  let resolve!: (preview: ReturnType<typeof historyPreview>) => void;
  read.mockImplementationOnce(
    () =>
      new Promise((finish) => {
        resolve = finish;
      }),
  );
  const pending = t.c.prepareHistory(1);
  t.c.state.sessions.set("second", {
    ...t.c.state.sessions.get("session-1")!,
    namespace: { ...t.view.namespace, sessionId: "second" },
  });
  vi.mocked(t.client.restore).mockImplementationOnce(async (id) => ({
    ...t.view,
    namespace: { ...t.view.namespace, sessionId: id },
  }));
  await t.c.select("second");
  resolve(historyPreview());
  await pending;
  expect(t.c.state.historyPreview).toBeUndefined();
  expect(t.c.state.history.size).toBe(0);
  t.c.dispose();
});
it("permits explicit across-process recovery from recovery_required, never auto-resumes on read and respects unknown/unsupported capabilities", async () => {
  const t = setup();
  await t.c.connect();
  await t.c.select("session-1");
  t.view.sessionStatus = "recovery_required";
  t.view.capabilities.continuation = "across_processes";
  t.emit();
  expect(t.c.canSend()).toBe(false);
  expect(t.c.canResume()).toBe(true);
  await t.c.restore();
  expect(t.client.resume).not.toHaveBeenCalled();
  await t.c.restore(true);
  expect(t.client.resume).toHaveBeenCalledExactlyOnceWith("session-1");
  expect(t.submit).not.toHaveBeenCalled();
  t.view.capabilities.continuation = "same_process";
  t.emit();
  expect(t.c.resumeReason()).toBe("context-unavailable");
  t.view.capabilities.continuation = "unknown";
  t.emit();
  expect(t.c.canResume()).toBe(false);
  t.view.capabilities.continuation = "unsupported";
  t.emit();
  expect(t.c.canResume()).toBe(false);
  t.c.dispose();
});

it("requires a fresh read of the old conversation after a connection reply arrives while another conversation is selected", async () => {
  const t = setup();
  await t.c.connect();
  await t.c.select("session-1");
  alternative(t);
  let finish!: (view: SessionView) => void;
  Object.assign(t.client, {
    selectConnection: vi.fn(
      () =>
        new Promise<SessionView>((resolve) => {
          finish = resolve;
        }),
    ),
  });
  const pending = t.c.chooseConnection("config-2");
  t.c.state.sessions.set("second", {
    ...t.c.state.sessions.get("session-1")!,
    namespace: { ...t.view.namespace, sessionId: "second" },
  });
  vi.mocked(t.client.restore).mockImplementationOnce(async (id) => ({
    ...structuredClone(t.view),
    namespace: { ...t.view.namespace, sessionId: id },
  }));
  await t.c.select("second");
  finish({ ...t.view, selectedConnectionId: "config-2" });
  await pending;
  expect(t.c.state.selected).toBe("second");
  expect(t.c.selectedConnectionId()).toBe("config-1");
  expect(t.c.state.views.get("session-1")!.connection).toBe("detached");
  vi.mocked(t.client.restore).mockResolvedValueOnce({
    ...t.view,
    selectedConnectionId: "config-2",
  });
  await t.c.select("session-1");
  expect(t.c.selectedConnectionId()).toBe("config-2");
  t.c.dispose();
});
it("keeps an uncertain resume fenced until a readonly original-session restore succeeds", async () => {
  const t = setup();
  await t.c.connect();
  await t.c.select("session-1");
  t.view.capabilities.continuation = "across_processes";
  t.emit();
  vi.mocked(t.client.resume).mockRejectedValueOnce(
    new ClientError("transport_failed"),
  );
  await t.c.restore(true);
  expect(t.c.state.optionPending.has("session-1")).toBe(true);
  expect(t.c.canResume()).toBe(false);
  expect(t.c.canSend()).toBe(false);
  await t.c.restore(true);
  expect(t.client.resume).toHaveBeenCalledOnce();
  await t.c.restore();
  expect(t.c.state.optionPending.size).toBe(0);
  expect(t.c.canSend()).toBe(true);
  expect(t.submit).not.toHaveBeenCalled();
  t.c.dispose();
});

it("requires an explicitly pending new context before previewing history and lets a recovering switched conversation explicitly start fresh", async () => {
  const t = setup();
  await t.c.connect();
  await t.c.select("session-1");
  alternative(t);
  const preview = vi.fn().mockResolvedValue(historyPreview());
  const choose = vi.fn(
    async (_id: string, connectionId: string, fresh: boolean) => ({
      ...deepClone(t.c.view()!),
      selectedConnectionId: connectionId,
      connectionPending: true,
      sessionStatus: fresh ? ("active" as const) : t.c.view()!.sessionStatus,
    }),
  );
  Object.assign(t.client, {
    previewHistory: preview,
    selectConnection: choose,
  });
  expect(await t.c.prepareHistory(1)).toBe(false);
  expect(preview).not.toHaveBeenCalled();
  t.view.sessionStatus = "recovery_required";
  t.emit();
  expect(await t.c.chooseConnection("config-2")).toBe(true);
  expect(t.c.canSend()).toBe(false);
  expect(t.c.resumeReason()).toBe("context-pending-recovery");
  expect(await t.c.chooseConnection("config-2", true)).toBe(true);
  expect(choose).toHaveBeenLastCalledWith("session-1", "config-2", true);
  expect(t.c.canSend()).toBe(true);
  expect(t.submit).not.toHaveBeenCalled();
  preview.mockResolvedValue(historyPreview("session-1", "config-2"));
  expect(await t.c.prepareHistory(1)).toBe(true);
  expect(t.c.confirmHistory()).toBe(true);
  t.c.dispose();
});
