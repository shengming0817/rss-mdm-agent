import { Context } from "@deepseek-ai/cordis";
import { SessionId } from "@deepseek-ai/dsh-session";
import { randomUUID } from "node:crypto";
import {
  assemble,
  COMPOSITION_ID,
  TOOL_PROFILE,
  type Initialization,
} from "./assembly.js";
import { copy, deferred } from "./support.js";
import { history, outcome } from "./history.js";
import { sealTools } from "./guard.js";
import type { NativeEvent, Operation } from "./protocol.js";
import { NativeFault } from "./protocol.js";

const ctx = new Context();
let init: Initialization | undefined,
  initialized = false,
  following = false,
  activeRequest: string | undefined;
let lastMessage = "";
let streamTask: Promise<void> | undefined;
const lifetime = new AbortController();
const callbacks = new Map<
  string,
  {
    kind: "question" | "proposal";
    resolve: (v: any) => void;
    reject: () => void;
    request?: any;
  }
>();
const notify = (event: NativeEvent) =>
  process.send?.(copy({ type: "event", event }));
const gateway = (method: string, request: unknown) =>
  ctx.typertGateway.invoke({
    namespace: "session",
    method,
    args: { request },
    signal: lifetime.signal,
  });
let verify = () => {};
let admittedAgent: ReturnType<typeof ctx.agents.get>;
let admittedSession: ReturnType<typeof ctx.sessions.get>;
let activation: string;
function drift() {
  if (lifetime.signal.aborted) return;
  notify({
    type: "lost",
    diagnostic: { stage: "profile", reason: "profile_drift" },
  });
  process.exitCode = 1;
  void shutdown();
}
function waitCallback(
  kind: "question" | "proposal",
  request: any,
  signal: AbortSignal,
): Promise<any> {
  if (signal.aborted || !activeRequest || callbacks.size >= 32)
    return Promise.reject(Error("callback unavailable"));
  const callbackId = randomUUID(),
    pending = deferred<any>();
  const abort = () => {
    if (callbacks.delete(callbackId)) {
      notify({
        type: "question_unavailable",
        requestId: activeRequest,
        callbackId,
      });
      pending.resolve({ unavailable: true });
    }
  };
  callbacks.set(callbackId, {
    kind,
    request,
    resolve: pending.resolve,
    reject: abort,
  });
  signal.addEventListener("abort", abort, { once: true });
  notify({
    type: kind === "question" ? "question" : "proposal",
    requestId: activeRequest,
    callbackId,
    ...(kind === "question" ? { request } : { proposal: request }),
  });
  return pending.promise
    .then((value) => {
      if (value.unavailable) throw Error("callback unavailable");
      return value;
    })
    .finally(() => {
      callbacks.delete(callbackId);
      signal.removeEventListener("abort", abort);
    });
}
async function initialize(i: Initialization) {
  if (init || i.composition !== COMPOSITION_ID)
    throw Error("invalid incarnation");
  init = copy(i);
  process.chdir(i.workingDirectory);
  const observed = await assemble(ctx, i, drift);
  activation = observed.activation;
  ctx.on("user-questions/request", async (request) => {
    verify();
    if (
      request.agent?.id !== i.nativeSessionId ||
      request.questions.some((q) => q.intent)
    )
      throw Error("unsupported permission request");
    return waitCallback(
      "question",
      { questions: request.questions },
      request.signal ?? lifetime.signal,
    );
  });
  if (i.controlled)
    ctx.tools.register({
      ...TOOL_PROFILE.proposal,
      output: {
        schema: { type: "object", additionalProperties: true },
        render: (_args, value) => [
          { type: "text", text: JSON.stringify(value) },
        ],
      },
      execute: async (args, exec) => {
        verify();
        return waitCallback("proposal", args, exec.signal);
      },
    });
  const allowed = [
    TOOL_PROFILE.question,
    ...(i.controlled ? [TOOL_PROFILE.proposal.name] : []),
  ];
  const verifyTools = sealTools(ctx, i.nativeSessionId, allowed, drift);
  verify = () => {
    observed.verify();
    verifyTools();
    if (
      (admittedSession &&
        ctx.sessions.get(SessionId(i.nativeSessionId)) !== admittedSession) ||
      (admittedAgent &&
        ctx.agents.get(SessionId(i.nativeSessionId)) !== admittedAgent)
    ) {
      drift();
      throw Error("native incarnation drift");
    }
  };
  ctx.on("agent/error", () => {
    if (activeRequest) notify({ type: "error", requestId: activeRequest });
  });
  ctx.on("llm/stream", (options, next) => {
    verify();
    return next();
  });
  let previousTerminal = false;
  if (i.restore) {
    const read = await ctx.sessionController.inspect(
      SessionId(i.nativeSessionId),
      lifetime.signal,
    );
    if (
      read.meta.id !== i.nativeSessionId ||
      read.meta.cwd !== i.workingDirectory ||
      ctx.agents.get(SessionId(i.nativeSessionId))
    )
      throw new NativeFault("restoration_failed");
    previousTerminal =
      !!i.previousRequestId &&
      history(read.events, i.previousRequestId).status === "terminal";
  } else
    await gateway("create", {
      sessionId: i.nativeSessionId,
      cwd: i.workingDirectory,
    });
  admittedSession = ctx.sessions.get(SessionId(i.nativeSessionId));
  admittedAgent = ctx.agents.get(SessionId(i.nativeSessionId));
  if ((!i.restore && !admittedSession) || !!admittedAgent === i.restore)
    throw Error("invalid activation");
  // Admission must survive close even when no first prompt ever triggers the
  // model checkpoint. Cold inspection remains strictly read-only.
  if (!i.restore) await ctx.sessions.flush(admittedSession!);
  verify();
  initialized = true;
  return {
    composition: COMPOSITION_ID,
    nativeSessionId: i.nativeSessionId,
    observationOnly: !ctx.agents.get(SessionId(i.nativeSessionId)),
    activation,
    previousTerminal,
  };
}
async function follow() {
  if (following) return;
  following = true;
  const stream = await ctx.typertGateway.stream({
    namespace: "session",
    method: "follow",
    args: {
      request: {
        address: { kind: "session", sessionId: init!.nativeSessionId },
        assistantStream: true,
      },
    },
    signal: lifetime.signal,
  });
  const iterator = stream[Symbol.asyncIterator]();
  const first = await iterator.next();
  if (first.done || (first.value as any).type !== "snapshot")
    throw Error("missing native snapshot");
  // This function is called only within an explicit submit. A cold follow may
  // now promote its observed context in this SAME admitted child Context.
  streamTask = (async () => {
    for (
      let item = await iterator.next();
      !item.done;
      item = await iterator.next()
    ) {
      const f: any = item.value;
      if (!activeRequest) continue;
      if (f.type === "assistant-stream") {
        const frame = f.frame;
        if (frame.type === "start") lastMessage = `message-${frame.attemptId}`;
        if (frame.type === "chunk" && frame.chunk.type === "text-delta")
          notify({
            type: "delta",
            requestId: activeRequest,
            messageId: lastMessage,
            text: frame.chunk.text,
          });
      } else if (f.type === "event") {
        const e = f.event;
        if (e.type === "assistant/message") {
          const text = e.data.message.content
            .filter((p: any) => p.type === "text")
            .map((p: any) => p.text)
            .join("");
          if (text)
            notify({
              type: "text",
              requestId: activeRequest,
              messageId: lastMessage || `message-${e.seq}`,
              text,
            });
        }
        if (e.type === "turn/end") {
          const ended = outcome(e.data.reason.kind);
          if (ended) {
            const session = ctx.sessions.get(SessionId(init!.nativeSessionId));
            if (!session) throw Error("lost native session");
            await ctx.sessions.flush(session);
            notify({
              type: "terminal",
              requestId: activeRequest,
              outcome: ended,
            });
            activeRequest = undefined;
          }
        }
      }
    }
    if (!lifetime.signal.aborted) notify({ type: "lost" });
  })().catch(() => {
    if (!lifetime.signal.aborted) {
      notify({ type: "lost" });
      void shutdown();
    }
  });
}
function answer(v: any) {
  const cb = callbacks.get(v.callbackId);
  if (!cb) throw Error("callback unavailable");
  if (v.unavailable) {
    cb.reject();
    return {};
  }
  if (cb.kind === "proposal") {
    cb.resolve(v.value);
    return {};
  }
  const questions = cb.request.questions,
    answer = v.answer;
  if (
    !answer ||
    !Array.isArray(answer.answers) ||
    answer.answers.length !== questions.length
  )
    throw Error("invalid answer");
  const ids = new Set();
  for (const row of answer.answers) {
    const q = questions.find((q: any) => q.id === row.id);
    if (
      !q ||
      ids.has(row.id) ||
      !Array.isArray(row.selected) ||
      row.selected.some(
        (s: any) =>
          typeof s !== "string" ||
          !(q.options ?? []).some((o: any) => o.label === s),
      ) ||
      (!q.multiSelect && row.selected.length > 1) ||
      (row.custom !== undefined && typeof row.custom !== "string")
    )
      throw Error("invalid answer");
    ids.add(row.id);
  }
  cb.resolve(copy(answer));
  return {};
}
async function shutdown() {
  lifetime.abort();
  for (const c of callbacks.values()) c.reject();
  await ctx.fiber.dispose();
  await streamTask;
}
const operations: Record<Operation, (value: any) => Promise<any> | any> = {
  initialize,
  async prompt(v) {
    verify();
    if (!initialized || activeRequest) throw Error("not ready");
    await follow();
    activeRequest = v.requestId;
    const value: any = await gateway("prompt", {
      sessionId: init!.nativeSessionId,
      requestId: v.requestId,
      mode: "queue",
      content: [{ type: "text", text: v.text }],
    });
    if (value.accepted !== true) throw Error("not accepted");
    const agent = ctx.agents.get(SessionId(init!.nativeSessionId));
    if (!agent || (admittedAgent && admittedAgent !== agent)) {
      drift();
      throw Error("invalid activation");
    }
    admittedAgent = agent;
    admittedSession ??= ctx.sessions.get(SessionId(init!.nativeSessionId));
    verify();
    return {
      status: "accepted",
      requestId: v.requestId,
      activation,
      observationOnly: false,
    };
  },
  async inspect(v) {
    verify();
    const cut = await ctx.sessionController.inspect(
      SessionId(init!.nativeSessionId),
      lifetime.signal,
    );
    const folded = history(cut.events, v.requestId);
    if (folded.status === "terminal") return folded;
    return activeRequest === v.requestId
      ? { status: "running" }
      : { status: "unknown" };
  },
  async cancel() {
    verify();
    return gateway("cancel", { sessionId: init!.nativeSessionId });
  },
  answer,
  tool_result: answer,
  async close() {
    await shutdown();
    return {};
  },
};
process.on("message", async (raw: any) => {
  const { id, operation, value } = raw;
  try {
    if (!Object.hasOwn(operations, operation)) throw Error("unknown operation");
    const result = await operations[operation as Operation](value);
    process.send?.(copy({ type: "reply", id, ok: true, value: result }));
  } catch (error) {
    process.send?.({
      type: "reply",
      id,
      ok: false,
      reason: error instanceof NativeFault ? error.reason : "native_failure",
    });
  }
});
process.on("disconnect", () => {
  void shutdown().finally(() => process.exit());
});
