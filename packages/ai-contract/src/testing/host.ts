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
} from "../ports.js";
import type { Command, Event, Receipt, Session } from "../wire.js";
import { fail, ok, MemorySessionStore } from "./store.js";
import { decode } from "../codec.js";
import { fixtureLimits } from "./store.js";
/** Script-driven acceptance/subscription fake. No provider, dispatcher, timer or durable storage. */
export class FakeHost implements HostPort {
  readonly evidence = "fake_host" as const;
  private next = 0;
  private listeners = new Map<string, Set<() => void>>();
  constructor(
    readonly store: SessionStore = new MemorySessionStore(),
    readonly clock: Clock = { now: () => 0 },
  ) {}
  negotiate(offered: Negotiation): Result<Negotiation> {
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
    if (budget.signal.aborted) return fail("unavailable");
    if (options.profile === "controlled_tools")
      return fail("permission_denied");
    const session: Session = {
      schemaVersion: 2,
      kind: "session",
      namespace: { ...caller, sessionId: `fake-session-${++this.next}` },
      revision: 0,
      lastSequence: 0,
      status: "active",
      binding: {
        provider: options.provider,
        providerVersion: "fixture-1",
        adapterVersion: "fixture-1",
        generation: `generation-${this.next}`,
        accountRef: options.accountRef,
        config: options.config,
        nativeSessionId: `native-${this.next}`,
      },
      capabilities: {
        continuation: "unsupported",
        cancellation: "request_only",
        tools: "disabled",
        steer: "unsupported",
        fork: "unsupported",
        subagent: "unsupported",
        terminal: "unsupported",
        structuredQuestion: "unsupported",
        multimodal: "unsupported",
      },
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
    if (budget.signal.aborted) return fail("unavailable");
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
    return this.store.snapshot(
      { ...caller, sessionId },
      _budget.maxSnapshotRecords ?? 1024,
    );
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
    let wake: (() => void) | undefined,
      changed = false;
    const onChange = () => {
      changed = true;
      wake?.();
    };
    const set = this.listeners.get(sessionId) ?? new Set();
    set.add(onChange);
    this.listeners.set(sessionId, set);
    budget.signal.addEventListener("abort", onChange);
    try {
      while (!budget.signal.aborted) {
        changed = false;
        const page = await this.store.events(
          { ...caller, sessionId },
          after,
          1024,
        );
        if (!page.ok) {
          yield { type: "resync_required" };
          return;
        }
        for (const event of page.value) {
          after = event.sequence;
          yield { type: "event", event };
        }
        if (page.value.length === 1024) continue;
        if (!changed && !budget.signal.aborted)
          await new Promise<void>((resolve) => {
            wake = resolve;
          });
        wake = undefined;
      }
    } finally {
      set.delete(onChange);
      if (!set.size) this.listeners.delete(sessionId);
      budget.signal.removeEventListener("abort", onChange);
    }
  }
}
