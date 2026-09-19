import { interactionCatalog } from "../identity.js";
import type {
  Budget,
  Caller,
  Clock,
  HostPort,
  Negotiation,
  Result,
  SessionOptions,
  SessionStore,
  Subscription,
  ProviderObservation,
} from "../ports.js";
import type {
  Command,
  Event,
  Receipt,
  Session,
  SurfaceState,
  Capabilities,
  Namespace,
  EventBody,
  Interaction,
  SnapshotPage,
  PageQuery,
  SessionPage,
} from "../wire.js";
import { fail, ok, MemorySessionStore, namespaceKey } from "./store.js";
import canonicalize from "canonicalize";
import { projectDelta } from "../protocol.js";
import { VerifiedProviderSession } from "../session.js";
import { ScriptedProvider } from "./provider.js";
import { withinBudget } from "./budget.js";
import { decode } from "../codec.js";
import { fixtureLimits } from "./store.js";
import { emptyCommit } from "./conformance.js";
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
    readonly scriptedCapabilities: Partial<Capabilities> = {},
  ) {}
  negotiate(offered: Negotiation): Result<Negotiation> {
    if (this.closed) return fail("unavailable");
    if (offered.contractVersion !== 2 || offered.acp !== 1)
      return fail("unsupported_version");
    if (
      offered.a2ui &&
      (offered.a2ui.version !== interactionCatalog.version ||
        offered.a2ui.catalogId !== interactionCatalog.catalogId ||
        offered.a2ui.catalogVersion !== interactionCatalog.catalogVersion)
    )
      return fail("unsupported_capability");
    return ok({ ...structuredClone(offered), durableReceipts: false });
  }
  async createSession(
    caller: Caller,
    options: SessionOptions,
    budget: Budget,
  ): Promise<Result<Session>> {
    if (this.closed || budget.signal.aborted) return fail("unavailable");
    if (options.profile === "controlled_tools")
      return fail("permission_denied");
    const provider = new ScriptedProvider();
    const admitted = await VerifiedProviderSession.open(
      provider,
      {
        provider: options.provider,
        config: options.config,
        accountRef: options.accountRef,
        workingDirectory: ".",
        permissions: "tools_disabled",
      },
      budget,
    );
    if (!admitted.ok) return admitted;
    this.providers.push(provider);
    if (this.closed) {
      await provider.close(budget);
      return fail("unavailable");
    }
    const session: Session = {
      schemaVersion: 2,
      kind: "session",
      namespace: { ...caller, sessionId: `fake-session-${++this.next}` },
      revision: 0,
      lastSequence: 0,
      status: "active",
      binding: admitted.value.binding,
      capabilities: {
        ...admitted.value.capabilities,
        queue: "supported",
        ...this.scriptedCapabilities,
        tools: "disabled",
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
    const prior = await this.store.command(namespace, command.commandId);
    if (!prior.ok) {
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
          (command.input.type === "cancel" &&
            command.input.nativeRunId !== s.binding.nativeRunId))
      )
        return fail("stale_binding");
      if (
        command.input.type === "prompt" &&
        command.input.policy === "queue_next" &&
        s.capabilities.queue !== "supported"
      ) {
        const page = await this.store.snapshotPage(namespace, { limit: 256 });
        if (!page.ok) return page;
        if (
          page.value.next ||
          page.value.commands.some(
            (record) =>
              record.command.input.type === "prompt" &&
              record.state !== "terminal",
          )
        )
          return fail("unsupported_capability");
      }
      if (command.input.type === "cancel") {
        if (s.capabilities.cancellation === "unsupported")
          return fail("unsupported_capability");
        const target = await this.store.command(
          namespace,
          command.input.targetCommandId,
        );
        if (!target.ok) return target;
      }
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
    if (result.ok) this.notify(s.namespace);
    return result;
  }
  surface(
    caller: Caller,
    sessionId: string,
    instanceId: string,
    budget: Budget,
  ): Promise<Result<SurfaceState>> {
    if (this.closed || budget.signal.aborted)
      return Promise.resolve(fail("unavailable"));
    return this.store.surface({ ...caller, sessionId }, instanceId);
  }
  snapshotPage(
    caller: Caller,
    sessionId: string,
    query: PageQuery,
    budget: Budget,
  ): Promise<Result<SnapshotPage>> {
    if (this.closed || budget.signal.aborted)
      return Promise.resolve(fail("unavailable"));
    return this.store.snapshotPage({ ...caller, sessionId }, query);
  }
  listSessions(
    caller: Caller,
    query: PageQuery,
    budget: Budget,
  ): Promise<Result<SessionPage>> {
    if (this.closed || budget.signal.aborted)
      return Promise.resolve(fail("unavailable"));
    return this.store.listSessions(caller, query);
  }
  async resume(
    caller: Caller,
    sessionId: string,
    budget: Budget,
  ): Promise<Result<Session>> {
    if (this.closed || budget.signal.aborted) return fail("unavailable");
    const found = await this.store.session({ ...caller, sessionId });
    if (!found.ok) return found;
    return found.value.capabilities.continuation === "same_process"
      ? found
      : fail("unsupported_capability");
  }
  async close(budget: Budget): Promise<Result<void>> {
    this.closed = true;
    for (const listeners of this.listeners.values())
      for (const wake of listeners) wake();
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
  /** Script-only provider evidence. Consumers explicitly choose all observations. */
  async advance(
    caller: Caller,
    sessionId: string,
    commandId: string,
    bodies: EventBody[],
  ): Promise<Result<void>> {
    const namespace = { ...caller, sessionId };
    const found = await this.store.session(namespace),
      row = await this.store.command(namespace, commandId);
    if (!found.ok) return found;
    if (!row.ok) return row;
    const s = found.value,
      record = row.value;
    const dispatch = record.dispatch ?? {
      generation: s.binding.generation,
      nativeSessionId: s.binding.nativeSessionId,
      nativeRunId: `run-${commandId}`,
      certainty: "submitted" as const,
    };
    if (!record.dispatch) {
      const started = await this.store.commit({
        ...emptyCommit(s),
        session: {
          ...s,
          revision: s.revision + 1,
          binding: { ...s.binding, nativeRunId: dispatch.nativeRunId },
        },
        commands: [{ ...record, state: "dispatching", dispatch }],
      });
      if (!started.ok) return started;
      return this.advance(caller, sessionId, commandId, bodies);
    }
    const terminal = bodies.find(
      (b): b is Extract<EventBody, { type: "terminal" }> =>
        b.type === "terminal",
    );
    const events: Event[] = bodies.map((body, index) => ({
      schemaVersion: 2,
      kind: "event",
      namespace,
      eventId: `script-${s.lastSequence + index + 1}`,
      sequence: s.lastSequence + index + 1,
      commandId,
      generation: s.binding.generation,
      body,
    }));
    const result = await this.store.commit({
      ...emptyCommit(s),
      session: {
        ...s,
        revision: s.revision + 1,
        lastSequence: s.lastSequence + events.length,
      },
      commands: terminal
        ? [{ ...record, state: "terminal", outcome: terminal.outcome }]
        : [],
      events,
    });
    if (result.ok) this.notify(namespace);
    return result;
  }
  async ask(
    caller: Caller,
    sessionId: string,
    commandId: string,
    request: Interaction["request"],
    interactionId = crypto.randomUUID(),
  ): Promise<Result<Interaction>> {
    const started = await this.advance(caller, sessionId, commandId, []);
    if (!started.ok) return started;
    const namespace = { ...caller, sessionId },
      found = await this.store.session(namespace);
    if (!found.ok) return found;
    const s = found.value;
    const interaction: Interaction = {
      schemaVersion: 2,
      kind: "interaction",
      category: "question",
      namespace,
      commandId,
      generation: s.binding.generation,
      nativeRunId: s.binding.nativeRunId,
      interactionId,
      nativeCallbackId: `callback-${interactionId}`,
      status: "pending",
      callbackLifetime: "generation_bound",
      expiresAtMs: this.clock.now() + 10000,
      request,
    };
    const result = await this.store.commit({
      ...emptyCommit(s),
      session: {
        ...s,
        revision: s.revision + 1,
        lastSequence: s.lastSequence + 1,
      },
      interactions: [interaction],
      events: [
        {
          schemaVersion: 2,
          kind: "event",
          namespace,
          eventId: `question-${interactionId}`,
          sequence: s.lastSequence + 1,
          commandId,
          generation: s.binding.generation,
          body: {
            type: "interaction",
            interactionId,
            status: "pending",
            expiresAtMs: interaction.expiresAtMs,
            callbackLifetime: interaction.callbackLifetime,
            request,
          },
        },
      ],
    });
    if (!result.ok) return result;
    this.notify(namespace);
    return ok(interaction);
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
    for (const wake of this.listeners.get(namespaceKey(namespace)) ?? []) {
      const queue = this.deltaQueues.get(wake)!;
      if (queue.length >= 1024)
        queue.splice(0, queue.length, { type: "resync_required" });
      else queue.push(projectDelta(observation));
      wake();
    }
    return ok(undefined);
  }
  /** Script harness calls this after committing fixture events, not to dispatch a provider. */
  notify(namespace: Namespace): void {
    for (const wake of this.listeners.get(namespaceKey(namespace)) ?? [])
      wake();
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
    const key = namespaceKey({ ...caller, sessionId });
    const set = this.listeners.get(key) ?? new Set();
    const deltas: Subscription[] = [];
    this.deltaQueues.set(onChange, deltas);
    set.add(onChange);
    this.listeners.set(key, set);
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
      if (!set.size) this.listeners.delete(key);
      budget.signal.removeEventListener("abort", onChange);
    }
  }
}
