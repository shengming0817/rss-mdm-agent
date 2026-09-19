import { randomUUID } from "node:crypto";
import {
  boundedJson,
  decode,
  fingerprint,
  isId,
} from "@rss-mdm-agent/ai-contract";
import {
  providerIdentity,
  workspaceIdentity,
} from "@rss-mdm-agent/ai-contract/session";
import type {
  Binding,
  Budget,
  Command,
  CommandRecord,
  DispatchAttempt,
  Outcome,
  ProviderAgentPort,
  ProviderConfiguration,
  ProviderObservation,
  ProviderSessionBinding,
  Reconciliation,
  Result,
  Submission,
} from "@rss-mdm-agent/ai-contract";
import {
  ADAPTER_VERSION,
  HARNESS_VERSION,
  API_URL,
  digest,
  identity,
  sessionPrefix,
  validateConfiguration,
  type DeepSeekAdapterOptions,
  type ResolvedDeepSeekConfiguration,
} from "./configuration.js";
import { COMPOSITION_ID } from "./assembly.js";
import { nativeRuntime } from "./runtime.js";
import type { NativeEvent, NativeRuntime, RuntimeFactory } from "./protocol.js";
import {
  bounded,
  copy,
  fail,
  limits,
  liveBudget,
  ok,
  Queue,
  ByteBudget,
  same,
} from "./support.js";
interface Callback {
  id: string;
  request: Record<string, unknown>;
  expires: number;
  answered?: string;
  timer: ReturnType<typeof setTimeout>;
}
interface Turn {
  command: Command;
  attempt: DispatchAttempt;
  binding: Binding;
  inputBinding: Binding;
  hash: string;
  accepted: boolean;
  uncertain: boolean;
  outcome?: Outcome;
  queue: Queue<ProviderObservation>;
  observing: boolean;
  callbacks: Map<string, Callback>;
}
export class DeepSeekAdapter implements ProviderAgentPort {
  private used = false;
  private closed = false;
  private failed = false;
  private runtime?: NativeRuntime;
  private initializing?: Promise<Result<ProviderSessionBinding>>;
  private binding?: Binding;
  private configuration?: ProviderConfiguration;
  private resolved?: ResolvedDeepSeekConfiguration;
  private turns = new Map<string, Turn>();
  private active?: Turn;
  private displayBudget = new ByteBudget(4 * 1024 * 1024);
  private restored = false;
  private unresolved = new Set<string>();
  private readonly lifetime = new AbortController();
  private readonly now: () => number;
  private readonly ttl: number;
  constructor(
    private readonly options: DeepSeekAdapterOptions,
    private readonly factory: RuntimeFactory = nativeRuntime,
  ) {
    this.now = () => options.clock?.now() ?? Date.now();
    this.ttl = options.callbackTimeoutMs ?? 120000;
    if (!Number.isSafeInteger(this.ttl) || this.ttl < 1 || this.ttl > 600000)
      throw Error("invalid callback timeout");
  }
  createSession(c: ProviderConfiguration, b: Budget) {
    return this.initialize(c, b);
  }
  resume(previous: Binding, c: ProviderConfiguration, b: Budget) {
    return this.initialize(c, b, previous);
  }
  private initialize(
    c: ProviderConfiguration,
    b: Budget,
    previous?: Binding,
  ): Promise<Result<ProviderSessionBinding>> {
    if (this.used || this.closed) return Promise.resolve(fail("stale_binding"));
    this.used = true;
    const task = this.open(c, b, previous);
    this.initializing = task;
    return task;
  }
  private async open(
    configuration: ProviderConfiguration,
    budget: Budget,
    previous?: Binding,
  ): Promise<Result<ProviderSessionBinding>> {
    try {
      const c = {
        ...configuration,
        namespace: copy(configuration.namespace),
        config: copy(configuration.config),
      };
      if (!liveBudget(budget)) return fail("unavailable");
      const start = Date.now();
      const remaining = () => ({
        ...budget,
        signal: AbortSignal.any([budget.signal, this.lifetime.signal]),
        timeoutMs: Math.max(0, budget.timeoutMs - (Date.now() - start)),
      });
      const resolved = await bounded(
        this.options.resolveConfiguration(
          { config: copy(c.config), accountRef: c.accountRef },
          remaining(),
        ),
        remaining(),
      );
      if (this.closed || !liveBudget(remaining())) return fail("unavailable");
      validateConfiguration(c, resolved);
      const prefix = sessionPrefix(c, resolved, COMPOSITION_ID);
      const providerVersion = `harness-${HARNESS_VERSION}.${COMPOSITION_ID}`;
      if (
        previous &&
        (previous.provider !== "deepseek" ||
          previous.providerVersion !== providerVersion ||
          previous.adapterVersion !== ADAPTER_VERSION ||
          previous.workspaceId !== workspaceIdentity(c.workingDirectory) ||
          previous.accountRef !== c.accountRef ||
          !same(previous.config, c.config) ||
          !previous.nativeSessionId.startsWith(prefix))
      )
        return fail("stale_binding");
      const binding: Binding = {
        provider: "deepseek",
        providerVersion,
        adapterVersion: ADAPTER_VERSION,
        config: copy(c.config),
        accountRef: c.accountRef,
        workspaceId: workspaceIdentity(c.workingDirectory),
        generation: randomUUID(),
        nativeSessionId: previous?.nativeSessionId ?? prefix + randomUUID(),
        ...(previous?.nativeRunId ? { nativeRunId: previous.nativeRunId } : {}),
        ...(previous?.nativeRequestId
          ? { nativeRequestId: previous.nativeRequestId }
          : {}),
      };
      this.configuration = c;
      this.resolved = { ...resolved, configuration: c };
      this.binding = binding;
      const runtime = this.factory();
      this.runtime = runtime;
      runtime.onEvent((e) => {
        try {
          this.event(e);
        } catch {
          this.invalidate();
        }
      });
      void runtime.stopped.then(
        () => this.lost(),
        () => this.lost(),
      );
      if (this.closed) {
        runtime.stop();
        return fail("unavailable");
      }
      const result = await runtime.call(
        "initialize",
        {
          nativeSessionId: binding.nativeSessionId,
          workingDirectory: c.workingDirectory,
          persistenceDirectory: resolved.persistenceDirectory,
          scope: digest(identity(c)),
          model: resolved.model,
          apiKey: resolved.apiKey,
          apiUrl: API_URL,
          controlled: c.permissions === "host_mediated",
          restore: !!previous,
          composition: COMPOSITION_ID,
        },
        remaining(),
      );
      if (
        this.closed ||
        this.failed ||
        !liveBudget(remaining()) ||
        result.composition !== COMPOSITION_ID ||
        result.nativeSessionId !== binding.nativeSessionId
      )
        throw Error("unverified incarnation");
      this.restored = !!previous;
      if (previous?.nativeRequestId)
        this.unresolved.add(previous.nativeRequestId);
      return ok({
        binding: copy(binding),
        capabilities: {
          continuation: "across_processes",
          cancellation: "request_only",
          tools:
            c.permissions === "host_mediated" ? "host_mediated" : "disabled",
          queue: "unsupported",
          steer: "unsupported",
          fork: "unsupported",
          subagent: "unsupported",
          terminal: "unsupported",
          structuredQuestion: "supported",
          multimodal: "unsupported",
        },
      });
    } catch {
      this.runtime?.stop();
      return fail("unavailable", "same_command");
    }
  }
  private current(b: Binding): boolean {
    return (
      !!this.binding &&
      !this.closed &&
      !this.failed &&
      same(providerIdentity(b), providerIdentity(this.binding))
    );
  }
  private checked(c: Command): Command {
    const value = decode(boundedJson(c, limits), limits);
    if (
      value.kind !== "command" ||
      value.sessionId !== this.configuration?.namespace.sessionId ||
      value.expiresAtMs <= this.now()
    )
      throw Error("invalid command");
    return value;
  }
  private requestId(c: Command, a: DispatchAttempt): string {
    return digest([
      this.configuration!.namespace,
      a.nativeSessionId,
      c.commandId,
      a.attemptId,
    ]);
  }
  async submit(
    binding: Binding,
    command: Command,
    attempt: DispatchAttempt,
    budget: Budget,
  ): Promise<Submission> {
    const denied = (
      code: Parameters<typeof fail>[0],
      retry: Parameters<typeof fail>[1] = "never",
    ): Submission => ({
      certainty: "not_sent",
      error: { code, retry: retry! },
    });
    if (!this.current(binding) || !liveBudget(budget))
      return denied("stale_binding");
    let c: Command;
    try {
      c = this.checked(command);
      decode(
        boundedJson(
          {
            schemaVersion: 2,
            kind: "event",
            namespace: this.configuration!.namespace,
            eventId: "validate",
            sequence: 1,
            commandId: c.commandId,
            generation: binding.generation,
            attemptId: attempt.attemptId,
            body: { type: "dispatch", attempt },
          },
          limits,
        ),
        limits,
      );
    } catch {
      return denied("invalid_input");
    }
    if (
      attempt.certainty !== "intent" ||
      attempt.originGeneration !== binding.generation ||
      attempt.observerGeneration !== binding.generation ||
      attempt.nativeSessionId !== binding.nativeSessionId ||
      attempt.nativeRequestId ||
      attempt.nativeRunId
    )
      return denied("stale_binding");
    if (c.input.type !== "prompt" || c.input.policy === "steer")
      return denied("unsupported_capability");
    const requestId = this.requestId(c, attempt),
      prior = this.turns.get(requestId);
    if (prior) {
      if (!same(attempt, prior.attempt)) return denied("stale_binding");
      if (prior.hash !== fingerprint(c, limits))
        return denied("content_conflict");
      return prior.accepted
        ? { certainty: "submitted", binding: copy(prior.binding) }
        : { certainty: "unknown", correlationId: requestId };
    }
    if (!same(binding, this.binding)) return denied("stale_binding");
    if (this.unresolved.size || (this.active && !this.active.outcome))
      return denied("reconciliation_required", "reconcile_first");
    if (this.turns.size >= 256) return denied("limit_exceeded");
    const live = { ...copy(binding), nativeRequestId: requestId };
    delete live.nativeRunId;
    const turn: Turn = {
      command: c,
      attempt: copy(attempt),
      binding: live,
      inputBinding: copy(binding),
      hash: fingerprint(c, limits),
      accepted: false,
      uncertain: false,
      queue: new Queue(1024, 2 * 1024 * 1024, this.displayBudget),
      observing: false,
      callbacks: new Map(),
    };
    this.turns.set(requestId, turn);
    this.active = turn;
    try {
      const reply = await this.runtime!.call(
        "prompt",
        { requestId, text: c.input.text },
        budget,
      );
      if (reply.requestId !== requestId || reply.status !== "accepted")
        throw Error("native prompt uncertain");
      this.accept(turn);
      return { certainty: "submitted", binding: copy(turn.binding) };
    } catch {
      turn.uncertain = true;
      this.unresolved.add(requestId);
      return { certainty: "unknown", correlationId: requestId };
    }
  }
  private accept(t: Turn) {
    if (!t.accepted) {
      t.accepted = true;
      this.binding = copy(t.binding);
      t.queue.push({
        type: "submitted",
        binding: copy(t.binding),
        commandId: t.command.commandId,
        attemptId: t.attempt.attemptId,
      });
    }
  }
  private emit(
    t: Turn,
    body: Extract<ProviderObservation, { type: "event" }>["body"],
  ) {
    t.queue.push({
      type: "event",
      binding: copy(t.binding),
      commandId: t.command.commandId,
      attemptId: t.attempt.attemptId,
      body,
    });
  }
  private event(e: NativeEvent): void {
    if (e.type === "lost") {
      this.lost();
      return;
    }
    if (this.closed || this.failed || !e.requestId) return;
    const t = this.turns.get(e.requestId);
    if (!t || t.outcome) throw Error("foreign native request");
    this.accept(t);
    if (e.type === "delta") {
      if (!isId(e.messageId) || typeof e.text !== "string")
        throw Error("invalid delta");
      t.queue.push({
        type: "delta",
        attemptId: t.attempt.attemptId,
        binding: copy(t.binding),
        commandId: t.command.commandId,
        messageId: e.messageId,
        text: e.text,
      });
    } else if (e.type === "text") {
      if (!isId(e.messageId) || typeof e.text !== "string")
        throw Error("invalid text");
      this.emit(t, { type: "text", messageId: e.messageId, text: e.text });
    } else if (e.type === "terminal") {
      if (
        !e.outcome ||
        ![
          "completed",
          "refused",
          "cancelled",
          "failed",
          "max_tokens",
          "max_turn_requests",
        ].includes(e.outcome)
      )
        throw Error("invalid terminal");
      this.invalidateCallbacks(t);
      t.outcome = e.outcome;
      this.unresolved.delete(e.requestId);
      this.emit(t, { type: "terminal", outcome: e.outcome });
      t.queue.end();
    } else if (e.type === "error")
      this.emit(t, {
        type: "error",
        failure: { code: "unavailable", retry: "never" },
      });
    else if (e.type === "question") {
      if (
        !isId(e.callbackId) ||
        !e.request ||
        t.callbacks.size >= 32 ||
        t.callbacks.has(e.callbackId)
      )
        throw Error("invalid question");
      const id = e.callbackId,
        expires = this.now() + this.ttl;
      const timer = setTimeout(() => {
        this.unavailable(t, id);
        void this.runtime
          ?.call(
            "answer",
            { callbackId: id, unavailable: true },
            { timeoutMs: 1000, signal: this.lifetime.signal },
          )
          .catch(() => {});
      }, this.ttl);
      timer.unref();
      t.callbacks.set(id, { id, request: copy(e.request), expires, timer });
      t.queue.push({
        type: "interaction",
        attemptId: t.attempt.attemptId,
        binding: copy(t.binding),
        commandId: t.command.commandId,
        interaction: {
          category: "question",
          interactionId: id,
          nativeCallbackId: id,
          expiresAtMs: expires,
          callbackLifetime: "generation_bound",
          request: copy(e.request),
        },
      });
    } else if (e.type === "question_unavailable")
      this.unavailable(t, e.callbackId!);
    else if (e.type === "proposal") void this.propose(t, e);
  }
  private async propose(t: Turn, e: NativeEvent) {
    if (
      this.configuration?.permissions !== "host_mediated" ||
      !e.callbackId ||
      !e.proposal ||
      !isId(e.proposal.name)
    ) {
      this.invalidate();
      return;
    }
    const proposalId = e.callbackId,
      b = {
        timeoutMs: 30000,
        signal: AbortSignal.any([
          this.lifetime.signal,
          AbortSignal.timeout(30000),
        ]),
      };
    this.emit(t, { type: "tool_proposal", proposalId, ...copy(e.proposal) });
    let value: {
      disposition: "returned" | "rejected" | "unavailable";
      text: string;
    } = { disposition: "unavailable", text: "Tool unavailable" };
    try {
      const result = await bounded(
        this.configuration.tools.propose(copy(e.proposal), b),
        b,
      );
      if (
        result.ok &&
        ["returned", "rejected", "unavailable"].includes(
          result.value.disposition,
        ) &&
        typeof result.value.text === "string"
      )
        value = copy(result.value);
    } catch {
      /* Native errors and secrets do not cross the port. */
    }
    if (this.closed || this.failed || t.outcome || this.active !== t) return;
    try {
      this.emit(t, { type: "tool_result", proposalId, ...value });
      await this.runtime!.call(
        "tool_result",
        { callbackId: proposalId, value },
        b,
      );
    } catch {
      this.invalidate();
    }
  }
  private unavailable(t: Turn, id: string) {
    const c = t.callbacks.get(id);
    if (!c || c.answered) return;
    clearTimeout(c.timer);
    t.callbacks.delete(id);
    t.queue.push({
      type: "interaction_unavailable",
      attemptId: t.attempt.attemptId,
      binding: copy(t.binding),
      commandId: t.command.commandId,
      interactionId: id,
    });
  }
  private invalidateCallbacks(t: Turn) {
    for (const id of t.callbacks.keys()) this.unavailable(t, id);
  }
  private lost() {
    this.failed = true;
    for (const t of this.turns.values()) {
      try {
        this.invalidateCallbacks(t);
      } catch {}
      t.uncertain = true;
      t.queue.end();
    }
  }
  private invalidate() {
    this.lost();
    this.runtime?.stop();
  }
  private turn(b: Binding) {
    return this.current(b)
      ? [...this.turns.values()].find(
          (t) => same(b, t.binding) || same(b, t.inputBinding),
        )
      : undefined;
  }
  async cancel(
    b: Binding,
    command: Command,
    budget: Budget,
  ): Promise<Result<"request_only" | "already_terminal">> {
    try {
      const c = this.checked(command),
        t = this.turn(b);
      if (
        !t ||
        c.input.type !== "cancel" ||
        c.input.targetCommandId !== t.command.commandId ||
        c.input.generation !== b.generation ||
        c.input.nativeRunId !== b.nativeRunId
      )
        return fail("stale_binding");
      if (t.outcome) return ok("already_terminal");
      await this.runtime!.call("cancel", {}, budget);
      return ok("request_only");
    } catch {
      return fail("unavailable", "reconcile_first");
    }
  }
  async respond(
    b: Binding,
    command: Command,
    budget: Budget,
  ): Promise<Result<void>> {
    try {
      const c = this.checked(command),
        t = this.turn(b);
      if (
        !t ||
        c.input.type !== "respond" ||
        c.input.generation !== b.generation ||
        c.input.nativeRunId !== b.nativeRunId
      )
        return fail("stale_binding");
      const cb = t.callbacks.get(c.input.interactionId);
      if (!cb || cb.expires <= this.now()) return fail("unavailable");
      const hash = fingerprint(c, limits);
      if (cb.answered)
        return cb.answered === hash ? ok(undefined) : fail("already_answered");
      await this.runtime!.call(
        "answer",
        { callbackId: cb.id, answer: c.input.answer },
        budget,
      );
      cb.answered = hash;
      clearTimeout(cb.timer);
      return ok(undefined);
    } catch {
      return fail("invalid_input");
    }
  }
  async *observe(
    b: Binding,
    budget: Budget,
  ): AsyncIterable<ProviderObservation> {
    const t = this.turn(b);
    if (!t || t.observing) return;
    t.observing = true;
    try {
      yield* t.queue.read(budget);
    } finally {
      t.observing = false;
    }
  }
  async reconcile(
    b: Binding,
    record: CommandRecord,
    budget: Budget,
  ): Promise<Result<Reconciliation>> {
    try {
      decode(boundedJson(record, limits), limits);
      const a = record.dispatch;
      if (
        !this.current(b) ||
        !a ||
        a.observerGeneration !== b.generation ||
        a.nativeSessionId !== b.nativeSessionId ||
        !same(record.receipt.namespace, this.configuration!.namespace) ||
        record.command.sessionId !== this.configuration!.namespace.sessionId ||
        ["terminal", "invalidated"].includes(record.state)
      )
        return fail("stale_binding");
      const requestId = this.requestId(record.command, a);
      if (
        (a.nativeRequestId && a.nativeRequestId !== requestId) ||
        (a.correlationId && a.correlationId !== requestId) ||
        a.nativeRunId
      )
        return fail("stale_binding");
      const t = this.turns.get(requestId);
      if (t && t.hash !== fingerprint(record.command, limits))
        return fail("content_conflict");
      const found = await this.runtime!.call("inspect", { requestId }, budget);
      const evidence = {
        commandId: record.command.commandId,
        attemptId: a.attemptId,
        binding: {
          ...copy(b),
          ...(a.nativeRequestId ? { nativeRequestId: a.nativeRequestId } : {}),
        },
      };
      // Missing history and an interrupted cold turn never prove non-submission.
      if (
        found.status === "terminal" &&
        [
          "completed",
          "refused",
          "cancelled",
          "failed",
          "max_tokens",
          "max_turn_requests",
        ].includes(found.outcome)
      ) {
        this.unresolved.delete(requestId);
        if (t) t.outcome = found.outcome;
        return ok({ ...evidence, status: "terminal", outcome: found.outcome });
      }
      if (
        found.status === "running" &&
        !this.restored &&
        t?.accepted &&
        !t.uncertain
      )
        return ok({ ...evidence, status: "running" });
      this.unresolved.add(requestId);
      return ok({ ...evidence, status: "unknown" });
    } catch {
      return fail("unavailable", "reconcile_first");
    }
  }
  async close(budget: Budget): Promise<Result<{ processStopped: boolean }>> {
    this.closed = true;
    this.lifetime.abort();
    this.lost();
    this.runtime?.stop();
    try {
      if (this.initializing) await bounded(this.initializing, budget);
      this.runtime?.stop();
      if (this.runtime) await bounded(this.runtime.stopped, budget);
      return ok({ processStopped: true });
    } catch {
      return fail("unavailable", "same_command");
    }
  }
}
