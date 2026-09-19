import { fingerprint, isId } from "@rss-mdm-agent/ai-contract";
import { randomUUID } from "node:crypto";
import { z } from "zod";
import type {
  CanUseTool,
  PermissionResult,
} from "@anthropic-ai/claude-agent-sdk";
import type {
  Binding,
  Command,
  ProviderObservation,
  Result,
} from "@rss-mdm-agent/ai-contract";
import { copy, deferred, fail, limits, ok, same } from "./support.js";
const questionSchema = z.object({
  questions: z
    .array(
      z.object({
        question: z.string().min(1).max(8192),
        header: z.string().max(128),
        options: z
          .array(
            z.object({
              label: z.string().min(1).max(1024),
              description: z.string().max(8192),
            }),
          )
          .min(2)
          .max(4),
        multiSelect: z.boolean(),
      }),
    )
    .min(1)
    .max(4),
});
const denied = (): PermissionResult => ({
  behavior: "deny",
  message: "Question unavailable",
});
type Callback = {
  binding: Binding;
  sessionId: string;
  request?: z.infer<typeof questionSchema>;
  expiresAt: number;
  status: "pending" | "answered" | "unavailable";
  responseHash?: string;
  settle: (result: PermissionResult) => void;
  invalidate: () => void;
};
/** Only question continuations live here. No permission or execution approval callback. */
export class Interactions {
  private callbacks = new Map<string, Callback>();
  private nativeIds = new Set<string>();
  constructor(
    private now: () => number,
    private ttl: number,
    private attemptId: string,
    private emit: (event: ProviderObservation) => void,
  ) {}
  ask(
    binding: Binding,
    command: Pick<Command, "sessionId" | "commandId">,
    input: Record<string, unknown>,
    options: Parameters<CanUseTool>[2],
  ): Promise<PermissionResult> {
    const parsed = questionSchema.safeParse(copy(input));
    if (
      !parsed.success ||
      options.signal.aborted ||
      !isId(options.requestId) ||
      !isId(options.toolUseID) ||
      this.nativeIds.has(options.requestId) ||
      this.callbacks.size >= 32
    )
      return Promise.resolve(denied());
    const request = parsed.data;
    if (
      new Set(request.questions.map((q) => q.question)).size !==
      request.questions.length
    )
      return Promise.resolve(denied());
    const interactionId = randomUUID(),
      expiresAt = this.now() + this.ttl,
      result = deferred<PermissionResult>();
    let timer: ReturnType<typeof setTimeout>;
    const callback: Callback = {
      binding: copy(binding),
      sessionId: command.sessionId,
      request,
      expiresAt,
      status: "pending",
      settle: (value) => {
        clearTimeout(timer);
        options.signal.removeEventListener("abort", callback.invalidate);
        result.resolve(value);
        callback.request = undefined;
        callback.settle = () => {};
        callback.invalidate = () => {};
      },
      invalidate: () => {
        if (callback.status !== "pending") return;
        callback.status = "unavailable";
        callback.settle(denied());
        this.emit({
          type: "interaction_unavailable",
          attemptId: this.attemptId,
          binding: copy(binding),
          commandId: command.commandId,
          interactionId,
        });
      },
    };
    this.nativeIds.add(options.requestId);
    this.callbacks.set(interactionId, callback);
    timer = setTimeout(callback.invalidate, this.ttl);
    timer.unref();
    options.signal.addEventListener("abort", callback.invalidate, {
      once: true,
    });
    this.emit({
      type: "interaction",
      attemptId: this.attemptId,
      binding: copy(binding),
      commandId: command.commandId,
      interaction: {
        category: "question",
        interactionId,
        nativeCallbackId: options.requestId,
        expiresAtMs: expiresAt,
        callbackLifetime: "generation_bound",
        request: copy(request),
      },
    });
    return result.promise;
  }
  respond(binding: Binding, command: Command): Result<void> {
    if (command.input.type !== "respond") return fail("invalid_input");
    const input = command.input,
      callback = this.callbacks.get(input.interactionId);
    if (!callback) return fail("unavailable");
    if (
      !same(callback.binding, binding) ||
      command.sessionId !== callback.sessionId ||
      input.generation !== binding.generation ||
      input.nativeRunId !== binding.nativeRunId
    )
      return fail("stale_binding");
    if (callback.status === "answered")
      return callback.responseHash === fingerprint(command, limits)
        ? ok(undefined)
        : fail("already_answered");
    if (callback.status !== "pending") return fail("unavailable");
    if (this.now() > callback.expiresAt) {
      callback.invalidate();
      return fail("expired");
    }
    const answer = z
      .object({ answers: z.record(z.string(), z.string().max(8192)) })
      .strict()
      .safeParse(copy(input.answer));
    if (
      !answer.success ||
      !same(
        Object.keys(answer.data.answers).sort(),
        callback.request!.questions.map((q) => q.question).sort(),
      )
    )
      return fail("invalid_input");
    callback.status = "answered";
    callback.responseHash = fingerprint(command, limits);
    callback.settle({
      behavior: "allow",
      updatedInput: { ...copy(callback.request), answers: answer.data.answers },
    });
    return ok(undefined);
  }
  invalidate(): void {
    for (const callback of this.callbacks.values()) callback.invalidate();
  }
}
