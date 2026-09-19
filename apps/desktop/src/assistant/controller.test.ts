import { describe, expect, it, vi } from "vitest";
import {
  ClientError,
  type ClientOptions,
  type RuntimeClient,
  type SessionView,
} from "@rss-mdm-agent/ai-client";
import { createAssistant } from "./controller";
import { emptyView } from "../../../../packages/ai-client/src/projection";
import { fixtureSession } from "../../../../packages/ai-contract/src/testing/conformance";
import fixtures from "../../../../tests/assistant/execution-fixtures.json";
import type { ExecutionTaskDetails } from "./execution-types";
function setup() {
  let next = 0,
    listener: (view: SessionView) => void = () => {},
    options: ClientOptions = {};
  const session = fixtureSession(),
    view = emptyView(session, 0);
  view.connection = "attached";
  const submit = vi.fn().mockResolvedValue({ kind: "receipt" });
  const client = {
    initialize: vi.fn().mockResolvedValue({ contractVersion: 3, acp: 1 }),
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
    () => 100,
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
  it("keeps drafts writable while capabilities independently gate send, steer, cancel and resume", async () => {
    const t = setup();
    await t.c.connect();
    await t.c.select("session-1");
    t.view.commands.p = {
      command: {
        schemaVersion: 3,
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
    expect(t.c.canSend.value).toBe(false);
    expect(t.c.canSteer.value).toBe(false);
    expect(t.c.canResume.value).toBe(false);
    await t.c.prompt();
    expect(t.submit).not.toHaveBeenCalled();
    t.view.capabilities.queue = "supported";
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
});
