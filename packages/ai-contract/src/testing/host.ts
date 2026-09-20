import type { PreferencesPatch } from "../wire.js";
import { historyPreview } from "../history.js";
import type {
  Connection,
  ConnectionPage,
  UserPreferences,
  HistoryPreview,
} from "../wire.js";
import { productSession } from "../contexts.js";
import { activeStage } from "../contexts.js";
import { verifiedReconciliation } from "./recovery.js";
import { readSnapshot } from "./snapshot.js";
import { interactionCatalog } from "../identity.js";
import type {
  Budget,
  Caller,
  Clock,
  HostPort,
  Negotiation,
  Result,
  SessionOptions,
  ConnectionOptions,
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
  Interaction,
  SnapshotPage,
  PageQuery,
  SessionPage,
} from "../wire.js";
import { fail, ok, MemorySessionStore, namespaceKey } from "./store.js";
import canonicalize from "canonicalize";
import { projectDelta } from "../protocol.js";
import { VerifiedProviderSession, providerIdentity } from "../session.js";
import { ScriptedProvider } from "./provider.js";
import { withinBudget } from "./budget.js";
import { decode } from "../codec.js";
import { fixtureLimits } from "./store.js";
import { emptyCommit, dispatchCommand } from "./conformance.js";
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
    if (offered.contractVersion !== 5 || offered.acp !== 1)
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
  async connections(
    caller: Caller,
    _budget: Budget,
  ): Promise<Result<ConnectionPage>> {
    const rows = await this.store.connections(caller),
      prefs = await this.store.preferences(caller);
    return rows.ok && prefs.ok
      ? ok({
          schemaVersion: 5,
          kind: "connectionPage",
          connections: [...rows.value],
          preferences: prefs.value,
        })
      : fail("unavailable");
  }
  saveConnection(
    caller: Caller,
    connection: Connection,
    expected: number | null,
    _budget: Budget,
  ) {
    return this.store.saveConnection(caller, connection, expected);
  }
  savePreferences(caller: Caller, prefs: PreferencesPatch, _budget: Budget) {
    return this.store.savePreferences(caller, prefs);
  }
  async selectConnection(
    caller: Caller,
    sessionId: string,
    connectionId: string,
    _budget: Budget,
    fresh = false,
  ) {
    const namespace = { ...caller, sessionId },
      session = await this.store.session(namespace);
    return session.ok
      ? this.store.selectConnection(
          namespace,
          connectionId,
          session.value.revision,
          fresh,
        )
      : session;
  }
  async previewHistory(
    caller: Caller,
    sessionId: string,
    connectionId: string,
    recent: number | undefined,
    _budget: Budget,
  ): Promise<Result<HistoryPreview>> {
    const snapshot = await readSnapshot(this.store, { ...caller, sessionId }),
      connection = await this.store.connection(caller, connectionId);
    return snapshot.ok && connection.ok
      ? ok(
          historyPreview(
            snapshot.value.session,
            snapshot.value.commands,
            snapshot.value.events,
            connection.value,
            recent,
          ),
        )
      : fail("unavailable");
  }
  async createSession(
    caller: Caller,
    options: SessionOptions,
    budget: Budget,
  ): Promise<Result<Session>> {
    if (this.closed || budget.signal.aborted) return fail("unavailable");
    if (Object.keys(options).some((key) => key !== "connectionId"))
      return fail("invalid_input");
    if (options.connectionId === "missing-connection")
      return fail("connection_required");
    const namespace = { ...caller, sessionId: `fake-session-${++this.next}` };
    const session = productSession(namespace, options.connectionId ?? "cfg");
    const created = await this.store.create(session);
    return created.ok ? ok(session) : created;
  }
  /** Explicit provider fixture setup for tests that exercise an already admitted context. */
  async openSessionForTest(
    caller: Caller,
    options: ConnectionOptions,
    budget: Budget,
  ): Promise<Result<Session>> {
    if (options.profile === "controlled_tools")
      return fail("permission_denied");
    const created = await FakeHost.prototype.createSession.call(
      this,
      caller,
      { connectionId: options.config.id },
      budget,
    );
    return created.ok ? this.open(created.value, budget, options) : created;
  }
  private async open(
    session: Session,
    budget: Budget,
    options?: ConnectionOptions,
  ): Promise<Result<Session>> {
    const provider = new ScriptedProvider(this.scriptedCapabilities);
    this.providers.push(provider);
    const admitted = await VerifiedProviderSession.open(
      provider,
      {
        provider: options?.provider ?? "fake",
        config: options?.config ?? {
          id: session.selectedConnectionId!,
          revision: "1",
        },
        accountRef: options?.accountRef ?? "account-1",
        workingDirectory: ".",
        namespace: session.namespace,
        permissions: "tools_disabled",
      },
      budget,
    );
    if (!admitted.ok) return admitted;
    const activated = await this.store.activateStage({
      namespace: session.namespace,
      expectedRevision: session.revision,
      configRevision: 1,
      credentialRevision: 1,
      opened: admitted.value,
    });
    if (activated.ok)
      for (const wake of this.listeners.get(namespaceKey(session.namespace)) ??
        []) {
        this.deltaQueues.get(wake)?.push({ type: "resync_required" });
        wake();
      }
    return activated;
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
    let s = found.value;
    const prior = await this.store.command(namespace, command.commandId);
    if (!prior.ok) {
      if (!s.currentStageId) {
        if (command.input.type !== "prompt") return fail("connection_required");
        const opened = await this.open(s, budget);
        if (!opened.ok) return opened;
        s = opened.value;
      }
      if (
        command.input.type === "prompt" &&
        command.input.policy === "steer" &&
        (activeStage(s).capabilities.steer !== "supported" ||
          command.input.targetRunId !== activeStage(s).binding.nativeRunId)
      )
        return fail("unsupported_capability");
      if (
        command.input.type !== "prompt" &&
        (command.input.generation !== activeStage(s).binding.generation ||
          (command.input.type === "cancel" &&
            command.input.nativeRunId !== activeStage(s).binding.nativeRunId))
      )
        return fail("stale_binding");
      if (command.input.type === "cancel") {
        if (
          ["unsupported", "unknown"].includes(
            activeStage(s).capabilities.cancellation,
          )
        )
          return fail("unsupported_capability");
        const target = await this.store.command(
          namespace,
          command.input.targetCommandId,
        );
        if (!target.ok) return target;
      }
    }
    const result = await this.store.accept({
      namespace,
      command,
      expectedRevision: s.revision,
      expectedGeneration: activeStage(s).binding.generation,
      nowMs: this.clock.now(),
      retention: { retryWindowMs: 1000, receiptWindowMs: 2000 },
      eventId: `accepted-${command.commandId}`,
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
    return activeStage(found.value).capabilities.continuation === "same_process"
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
    bodies: Event["body"][],
  ): Promise<Result<void>> {
    const namespace = { ...caller, sessionId };
    const found = await this.store.session(namespace),
      row = await this.store.command(namespace, commandId);
    if (!found.ok) return found;
    if (!row.ok) return row;
    const s = found.value,
      record = row.value;
    if (!record.dispatch) {
      try {
        await dispatchCommand(
          this.store,
          s,
          commandId,
          "submitted",
          {
            nativeRunId: `run-${commandId}`,
          },
          this.clock.now(),
        );
      } catch {
        return fail("invalid_input");
      }
      return this.advance(caller, sessionId, commandId, bodies);
    }
    const terminal = bodies.find(
      (b): b is Extract<Event["body"], { type: "terminal" }> =>
        b.type === "terminal",
    );
    if (terminal && bodies.length > 1) {
      if (bodies.at(-1) !== terminal) return fail("invalid_input");
      const observed = await this.advance(
        caller,
        sessionId,
        commandId,
        bodies.slice(0, -1),
      );
      return observed.ok
        ? this.advance(caller, sessionId, commandId, [terminal])
        : observed;
    }
    const events: Event[] = bodies.map(
      (body, index) =>
        ({
          schemaVersion: 5,
          kind: "event",
          namespace,
          eventId: `script-${s.lastSequence + index + 1}`,
          sequence: s.lastSequence + index + 1,
          commandId,
          attemptId: record.dispatch!.attemptId,
          generation: activeStage(s).binding.generation,
          body,
        }) as Event,
    );
    const interactions: Interaction[] = [],
      surfaces: SurfaceState[] = [];
    if (terminal) {
      const snapshot = await readSnapshot(this.store, namespace);
      if (!snapshot.ok) return snapshot;
      for (const row of snapshot.value.interactions)
        if (row.commandId === commandId && row.status === "pending") {
          interactions.push({ ...row, status: "unavailable" });
          events.push({
            schemaVersion: 5,
            kind: "event",
            namespace,
            eventId: `script-${s.lastSequence + events.length + 1}`,
            sequence: s.lastSequence + events.length + 1,
            commandId,
            attemptId: record.dispatch.attemptId,
            generation: activeStage(s).binding.generation,
            body: {
              type: "interaction",
              interactionId: row.interactionId,
              status: "unavailable",
            },
          });
        }
      for (const row of snapshot.value.surfaces)
        if (
          row.status === "active" &&
          snapshot.value.interactions.some(
            (i) =>
              i.interactionId === row.interactionId &&
              i.commandId === commandId,
          )
        ) {
          const surface: SurfaceState = {
            ...row,
            status: "invalidated",
            revision: row.revision + 1,
          };
          surfaces.push(surface);
          events.push({
            schemaVersion: 5,
            kind: "event",
            namespace,
            eventId: `script-${s.lastSequence + events.length + 1}`,
            sequence: s.lastSequence + events.length + 1,
            commandId,
            attemptId: record.dispatch.attemptId,
            generation: activeStage(s).binding.generation,
            body: { type: "surface", surface },
          });
        }
    }
    const providerFacts = terminal
      ? [await verifiedReconciliation(s, record, "terminal", terminal.outcome)]
      : [];
    if (terminal)
      events.push({
        schemaVersion: 5,
        kind: "event",
        namespace,
        eventId: `script-proof-${s.revision}`,
        sequence: s.lastSequence + events.length + 1,
        commandId,
        attemptId: record.dispatch!.attemptId,
        generation: activeStage(s).binding.generation,
        body: {
          type: "reconciled",
          attempt: record.dispatch!,
          resolution: "terminal",
        },
      });
    const result = await this.store.commit({
      providerFacts,
      ...emptyCommit(s),
      session: {
        ...s,
        revision: s.revision + 1,
        lastSequence: s.lastSequence + events.length,
      },
      commands: terminal
        ? [
            {
              schemaVersion: 5,
              kind: "commandRecord",
              command: record.command,
              receipt: record.receipt,
              dispatch: record.dispatch!,
              state: "terminal",
              outcome: terminal.outcome,
            },
          ]
        : [],
      events,
      interactions,
      surfaces,
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
    let s = found.value;
    const interaction: Interaction = {
      schemaVersion: 5,
      kind: "interaction",
      category: "question",
      namespace,
      commandId,
      generation: activeStage(s).binding.generation,
      nativeRunId: activeStage(s).binding.nativeRunId,
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
          schemaVersion: 5,
          kind: "event",
          namespace,
          eventId: `question-${interactionId}`,
          attemptId: `attempt-${commandId}`,
          sequence: s.lastSequence + 1,
          commandId,
          generation: activeStage(s).binding.generation,
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
      canonicalize(providerIdentity(activeStage(session.value).binding)) !==
      canonicalize(providerIdentity(observation.binding))
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
      dispatch.nativeThreadId !== observation.binding.nativeThreadId ||
      dispatch.nativeRunId !== observation.binding.nativeRunId ||
      dispatch.nativeRequestId !== observation.binding.nativeRequestId
    )
      return fail("stale_binding");
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
