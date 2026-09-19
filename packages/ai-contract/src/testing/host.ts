import type {
  Budget,
  Caller,
  Clock,
  HostPort,
  Negotiation,
  Result,
  SessionOptions,
  SessionStore,
  Snapshot,
  Subscription,
  ProviderObservation,
} from "../ports.js";
import type { Command, Event, Receipt, Session } from "../wire.js";
import { fail, ok, MemorySessionStore } from "./store.js";
import canonicalize from "canonicalize";
import { projectDelta } from "../protocol.js";
import { VerifiedProviderSession } from "../session.js";
import { ScriptedProvider } from "./provider.js";
import { withinBudget } from "./budget.js";
import { decode } from "../codec.js";
import { fixtureLimits } from "./store.js";
/** Script-driven acceptance/subscription fake with scripted ports only. No process, dispatcher or durable storage. */
export class FakeHost implements HostPort {
  readonly evidence = "fake_host" as const;
  private next = 0;
  private closed = false;
  private providers: ScriptedProvider[] = [];
  private deltaQueues = new Map<() => void, Subscription[]>();
  private listeners = new Map<string, Set<() => void>>();
  constructor(
    readonly store: SessionStore = new MemorySessionStore(),
    readonly clock: Clock = { now: () => 0 },
  ) {}
  negotiate(offered: Negotiation): Result<Negotiation> {
    if (this.closed) return fail("unavailable");
    if (offered.contractVersion !== 2 || offered.acp !== 1)
      return fail("unsupported_version");
    if (
      offered.a2ui &&
      (offered.a2ui.version !== "v0.9.1" ||
        !offered.a2ui.catalogId ||
        !offered.a2ui.catalogVersion)
    )
      return fail("unsupported_capability");
    return ok(structuredClone(offered));
  }
  async createSession(
    caller: Caller,
    options: SessionOptions,
    budget: Budget,
  ): Promise<Result<Session>> {
    if (this.closed || budget.signal.aborted) return fail("unavailable");
    if (options.profile === "controlled_tools")
      return fail("permission_denied");
    const namespace = { ...caller, sessionId: `fake-session-${++this.next}` };
    const provider = new ScriptedProvider();
    this.providers.push(provider);
    const admitted = await VerifiedProviderSession.open(
      provider,
      {
        provider: options.provider,
        config: options.config,
        accountRef: options.accountRef,
        workingDirectory: ".",
        namespace,
        permissions: "tools_disabled",
      },
      budget,
    );
    if (!admitted.ok) return admitted;
    if (this.closed) {
      await provider.close(budget);
      return fail("unavailable");
    }
    const session: Session = {
      schemaVersion: 2,
      kind: "session",
      namespace,
      revision: 0,
      lastSequence: 0,
      status: "active",
      binding: admitted.value.binding,
      capabilities: admitted.value.capabilities,
    };
    const result = await this.store.create(session);
    return result.ok ? ok(session) : result;
  }
  submit(
    caller: Caller,
    command: Command,
    budget: Budget,
  ): Promise<Result<Receipt>> {
    return command.input.type === "prompt"
      ? this.accept(caller, command, budget)
      : Promise.resolve(fail("invalid_input"));
  }
  cancel(
    caller: Caller,
    command: Command,
    budget: Budget,
  ): Promise<Result<Receipt>> {
    return command.input.type === "cancel"
      ? this.accept(caller, command, budget)
      : Promise.resolve(fail("invalid_input"));
  }
  respond(
    caller: Caller,
    command: Command,
    budget: Budget,
  ): Promise<Result<Receipt>> {
    return command.input.type === "respond"
      ? this.accept(caller, command, budget)
      : Promise.resolve(fail("invalid_input"));
  }
  private async accept(
    caller: Caller,
    command: Command,
    budget: Budget,
  ): Promise<Result<Receipt>> {
    if (this.closed || budget.signal.aborted) return fail("unavailable");
    try {
      decode(JSON.stringify(command), fixtureLimits);
    } catch {
      return fail("invalid_input");
    }
    const namespace = { ...caller, sessionId: command.sessionId };
    const found = await this.store.session(namespace);
    if (!found.ok) return found;
    const s = found.value;
    if (
      command.input.type === "prompt" &&
      command.input.policy === "steer" &&
      (s.capabilities.steer !== "supported" ||
        command.input.targetRunId !== s.binding.nativeRunId)
    )
      return fail("unsupported_capability");
    if (
      command.input.type !== "prompt" &&
      (command.input.generation !== s.binding.generation ||
        command.input.nativeRunId !== s.binding.nativeRunId)
    )
      return fail("stale_binding");
    if (command.input.type === "cancel") {
      const target = await this.store.command(
        namespace,
        command.input.targetCommandId,
      );
      if (!target.ok) return target;
    }
    const event: Event = {
      schemaVersion: 2,
      kind: "event",
      namespace,
      eventId: `accepted-${command.commandId}`,
      sequence: s.lastSequence + 1,
      commandId: command.commandId,
      generation: s.binding.generation,
      body: { type: "status", state: "accepted" },
    };
    const result = await this.store.accept({
      namespace,
      command,
      expectedRevision: s.revision,
      expectedGeneration: s.binding.generation,
      nowMs: this.clock.now(),
      retention: { retryWindowMs: 1000, receiptWindowMs: 2000 },
      event,
    });
    if (result.ok) this.notify(s.namespace.sessionId);
    return result;
  }
  snapshot(
    caller: Caller,
    sessionId: string,
    _budget: Budget,
  ): Promise<Result<Snapshot>> {
    if (this.closed || _budget.signal.aborted)
      return Promise.resolve(fail("unavailable"));
    return this.store.snapshot(
      { ...caller, sessionId },
      _budget.maxSnapshotRecords ?? 1024,
    );
  }
  async close(budget: Budget): Promise<Result<void>> {
    this.closed = true;
    for (const sessionId of this.listeners.keys()) this.notify(sessionId);
    this.listeners.clear();
    this.deltaQueues.clear();
    try {
      return await withinBudget(
        () => budget,
        async (b) => {
          let failed = false;
          for (const provider of this.providers) {
            try {
              const result = await provider.close(b);
              failed ||= !result.ok || !result.value.processStopped;
            } catch {
              failed = true;
            }
          }
          try {
            const result = await this.store.close(b);
            failed ||= !result.ok;
          } catch {
            failed = true;
          }
          return failed ? fail("unavailable", "same_command") : ok(undefined);
        },
      );
    } catch {
      return fail("unavailable", "same_command");
    }
  }
  /** Explicit scripted stimulus; never a native provider/security proof. */
  async publishDelta(
    caller: Caller,
    sessionId: string,
    observation: Extract<ProviderObservation, { type: "delta" }>,
  ): Promise<Result<void>> {
    if (this.closed) return fail("unavailable");
    const namespace = { ...caller, sessionId };
    const session = await this.store.session(namespace);
    if (!session.ok) return session;
    if (
      canonicalize(session.value.binding) !== canonicalize(observation.binding)
    )
      return fail("stale_binding");
    const command = await this.store.command(namespace, observation.commandId);
    if (!command.ok) return command;
    const record = command.value,
      dispatch = record.dispatch;
    if (
      !dispatch ||
      !["dispatching", "running"].includes(record.state) ||
      dispatch.certainty !== "submitted" ||
      dispatch.attemptId !== observation.attemptId ||
      dispatch.observerGeneration !== observation.binding.generation ||
      dispatch.nativeSessionId !== observation.binding.nativeSessionId ||
      dispatch.nativeRunId !== observation.binding.nativeRunId ||
      dispatch.nativeRequestId !== observation.binding.nativeRequestId
    )
      return fail("stale_binding");
    for (const wake of this.listeners.get(sessionId) ?? []) {
      const queue = this.deltaQueues.get(wake)!;
      if (queue.length >= 1024)
        queue.splice(0, queue.length, { type: "resync_required" });
      else queue.push(projectDelta(observation));
      wake();
    }
    return ok(undefined);
  }
  /** Script harness calls this after committing fixture events, not to dispatch a provider. */
  notify(sessionId: string): void {
    for (const wake of this.listeners.get(sessionId) ?? []) wake();
  }
  async *subscribe(
    caller: Caller,
    sessionId: string,
    after: number,
    budget: Budget,
  ): AsyncIterable<Subscription> {
    if (this.closed) return;
    let wake: (() => void) | undefined,
      changed = false;
    const onChange = () => {
      changed = true;
      wake?.();
    };
    const set = this.listeners.get(sessionId) ?? new Set();
    const deltas: Subscription[] = [];
    this.deltaQueues.set(onChange, deltas);
    set.add(onChange);
    this.listeners.set(sessionId, set);
    budget.signal.addEventListener("abort", onChange);
    try {
      while (!this.closed && !budget.signal.aborted) {
        changed = false;
        const page = await this.store.events(
          { ...caller, sessionId },
          after,
          1024,
        );
        if (this.closed || budget.signal.aborted) return;
        if (!page.ok) {
          yield { type: "resync_required" };
          return;
        }
        for (const event of page.value) {
          if (this.closed || budget.signal.aborted) return;
          after = event.sequence;
          yield { type: "event", event };
        }
        while (deltas.length && !this.closed && !budget.signal.aborted)
          yield deltas.shift()!;
        if (page.value.length === 1024) continue;
        if (!changed && !this.closed && !budget.signal.aborted)
          await new Promise<void>((resolve) => {
            wake = resolve;
          });
        wake = undefined;
      }
    } finally {
      this.deltaQueues.delete(onChange);
      set.delete(onChange);
      if (!set.size) this.listeners.delete(sessionId);
      budget.signal.removeEventListener("abort", onChange);
    }
  }
}
