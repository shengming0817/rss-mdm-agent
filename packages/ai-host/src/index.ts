import { randomUUID } from "node:crypto";
import {
  boundedJson,
  decode,
  interactionCatalog,
  type Budget,
  type Caller,
  type Command,
  type CommandRecord,
  type Event,
  type HostPort,
  type Interaction,
  type Namespace,
  type Negotiation,
  type PageQuery,
  type ProviderAdmission,
  type ProviderConfiguration,
  type Receipt,
  type Result,
  type Session,
  type SessionCommit,
  type SessionOptions,
  type SessionStore,
  type SnapshotPage,
  type Subscription,
  type SurfaceState,
  type VerifiedProviderFact,
} from "@rss-mdm-agent/ai-contract";
import {
  VerifiedProviderSession,
  providerFactFor,
} from "@rss-mdm-agent/ai-contract/session";
import {
  defaultLimits,
  controlIsStale,
  fail,
  isSettled,
  namespaceKey,
  ok,
} from "@rss-mdm-agent/ai-contract/transitions";
import { WorkerPort, groupEmpty } from "./process.js";
import { Output } from "./queue.js";

export interface HostDiagnostic {
  readonly stage: "admission" | "dispatch" | "observe" | "recovery" | "close";
  readonly code: import("@rss-mdm-agent/ai-contract").Failure["code"];
}
export interface HostOptions {
  readonly onDiagnostic?: (diagnostic: HostDiagnostic) => void;
  readonly store: SessionStore;
  /** Trusted composition. Account references are not credentials. */
  resolve(
    caller: Caller,
    options: SessionOptions,
    namespace: Namespace,
    budget: Budget,
  ): Promise<{
    configuration: ProviderConfiguration;
    artifact: string;
    admission?: ProviderAdmission;
  }>;
  readonly queueLimit?: number;
  readonly workerLimit?: number;
  readonly accountWorkerLimit?: number;
  readonly operationTimeoutMs?: number;
  readonly now?: () => number;
}
interface Runtime {
  worker: WorkerPort;
  verified?: VerifiedProviderSession;
  account: string;
  observing: Set<string>;
  inFlight: Set<string>;
  controlBurst: number;
  retryAfter: Map<string, number>;
  abort: AbortController;
}
const budget = (timeoutMs = 30000): Budget => ({
  timeoutMs,
  signal: new AbortController().signal,
});
const baseRecord = (record: CommandRecord) => ({
  schemaVersion: 2 as const,
  kind: "commandRecord" as const,
  command: record.command,
  receipt: record.receipt,
});
const isOrdinary = (record: CommandRecord) =>
  record.command.input.type === "prompt" &&
  record.command.input.policy === "queue_next";
const same = (a: unknown, b: unknown) =>
  JSON.stringify(a) === JSON.stringify(b);
const requireValue = <T>(result: Result<T>): T => {
  if (!result.ok) throw new HostFailure(result.error);
  return result.value;
};
class HostFailure extends Error {
  constructor(readonly failure: import("@rss-mdm-agent/ai-contract").Failure) {
    super(failure.code);
  }
}

const hostOwners = new WeakSet<SessionStore>();

/** One database owner and one short-lived mailbox per logical session. */
export class SessionHost implements HostPort {
  private readonly mailboxes = new Map<string, Promise<unknown>>();
  private readonly runtimes = new Map<string, Runtime>();
  private readonly subscribers = new Map<string, Set<Output<Subscription>>>();
  private readonly tasks = new Set<Promise<unknown>>();
  private readonly retryTimers = new Set<ReturnType<typeof setTimeout>>();
  private readonly blocked = new Set<string>();
  private readonly admissions = new Map<Promise<unknown>, AbortController>();
  private closingTask?: Promise<Result<void>>;
  private closing = false;
  private closed = false;
  private readonly now: () => number;
  private readonly queueLimit: number;
  private readonly workerLimit: number;
  private readonly accountLimit: number;
  private readonly timeout: number;
  private constructor(private readonly options: HostOptions) {
    this.now = options.now ?? Date.now;
    this.queueLimit = options.queueLimit ?? 64;
    this.workerLimit = options.workerLimit ?? 8;
    this.accountLimit = options.accountWorkerLimit ?? 2;
    this.timeout = options.operationTimeoutMs ?? 30000;
    if (
      [this.queueLimit, this.workerLimit, this.accountLimit, this.timeout].some(
        (n) => !Number.isSafeInteger(n) || n < 1,
      )
    )
      throw new TypeError("Host limits");
  }
  static async create(options: HostOptions): Promise<Result<SessionHost>> {
    if (hostOwners.has(options.store)) return fail("unavailable");
    const host = new SessionHost(options);
    hostOwners.add(options.store);
    const started = await host.start();
    if (!started.ok) await host.close(budget());
    return started.ok ? ok(host) : started;
  }
  private diagnose(stage: HostDiagnostic["stage"], error: unknown) {
    try {
      this.options.onDiagnostic?.({
        stage,
        code: error instanceof HostFailure ? error.failure.code : "unavailable",
      });
    } catch {}
  }
  private get store() {
    return this.options.store;
  }
  private mailbox<T>(
    namespace: Namespace,
    action: () => Promise<T>,
  ): Promise<T> {
    const key = namespaceKey(namespace),
      previous = this.mailboxes.get(key) ?? Promise.resolve();
    const next = previous.catch(() => {}).then(action);
    this.mailboxes.set(key, next);
    void next
      .finally(() => {
        if (this.mailboxes.get(key) === next) this.mailboxes.delete(key);
      })
      .catch(() => {});
    return next;
  }
  private track(task: Promise<unknown>) {
    this.tasks.add(task);
    void task.finally(() => this.tasks.delete(task)).catch(() => {});
  }
  private admit<T>(
    namespace: Namespace,
    b: Budget,
    action: (budget: Budget) => Promise<Result<T>>,
  ): Promise<Result<T>> {
    const abort = new AbortController();
    const task = this.mailbox(namespace, () =>
      this.closing
        ? Promise.resolve(fail<T>("unavailable"))
        : action({ ...b, signal: AbortSignal.any([b.signal, abort.signal]) }),
    );
    this.admissions.set(task, abort);
    void task.finally(() => this.admissions.delete(task)).catch(() => {});
    return task;
  }
  private async result<T>(
    action: () => Promise<Result<T>>,
  ): Promise<Result<T>> {
    try {
      return await action();
    } catch (error) {
      return error instanceof HostFailure
        ? { ok: false, error: error.failure }
        : fail("unavailable", "same_command");
    }
  }
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
    return ok({ ...structuredClone(offered), durableReceipts: true });
  }
  /** Scan launch fences before any provider restore. Saved process IDs never authorize signaling. */
  private async start(): Promise<Result<void>> {
    return this.result(async () => {
      for (const launch of requireValue(await this.store.launches())) {
        if (launch.phase === "reserved" || groupEmpty(launch.pgid))
          requireValue(
            await this.store.releaseLaunch(launch.namespace, launch.launchId),
          );
        else {
          this.blocked.add(namespaceKey(launch.namespace));
          await this.unavailable(launch.namespace);
        }
      }
      const namespaces = new Map<string, Namespace>();
      let after: string | undefined;
      do {
        const page = requireValue(await this.store.recovery(128, after));
        for (const record of page.items)
          namespaces.set(
            namespaceKey(record.receipt.namespace),
            record.receipt.namespace,
          );
        after = page.next;
      } while (after);
      for (const namespace of namespaces.values())
        await this.resume(namespace, namespace.sessionId, budget(this.timeout));
      return ok(undefined);
    });
  }
  private async open(
    namespace: Namespace,
    options: SessionOptions,
    b: Budget,
    previous?: Session,
  ): Promise<Result<Session>> {
    if (this.closing || b.signal.aborted) return fail("unavailable");
    // Retry orphan collection on admission, without a heartbeat or signaling old PIDs.
    for (const launch of requireValue(await this.store.launches())) {
      const blockedKey = namespaceKey(launch.namespace);
      if (
        this.blocked.has(blockedKey) &&
        (launch.phase === "reserved" || groupEmpty(launch.pgid))
      ) {
        requireValue(
          await this.store.releaseLaunch(launch.namespace, launch.launchId),
        );
        this.blocked.delete(blockedKey);
      }
    }
    const key = namespaceKey(namespace);
    if (this.runtimes.has(key))
      return fail("reconciliation_required", "reconcile_first");
    if (this.blocked.has(key)) {
      const fence = requireValue(await this.store.launches()).find(
        (row) => namespaceKey(row.namespace) === key,
      );
      if (fence?.phase === "registered" && !groupEmpty(fence.pgid))
        return fail("reconciliation_required", "reconcile_first");
      if (fence)
        requireValue(await this.store.releaseLaunch(namespace, fence.launchId));
      this.blocked.delete(key);
    }
    const resolved = await this.options.resolve(
        namespace,
        options,
        namespace,
        b,
      ),
      configuration = structuredClone(resolved.configuration);
    if (this.closing || b.signal.aborted) return fail("unavailable");
    if (
      namespaceKey(configuration.namespace) !== namespaceKey(namespace) ||
      configuration.provider !== options.provider ||
      configuration.config.id !== options.config.id ||
      configuration.config.revision !== options.config.revision ||
      configuration.accountRef !== options.accountRef ||
      configuration.permissions !==
        (options.profile === "controlled_tools"
          ? "host_mediated"
          : "tools_disabled")
    )
      return fail("permission_denied");
    const account = JSON.stringify([
      configuration.provider,
      configuration.accountRef,
    ]);
    if (
      this.runtimes.size >= this.workerLimit ||
      [...this.runtimes.values()].filter((r) => r.account === account).length >=
        this.accountLimit
    )
      return fail("limit_exceeded");
    const worker = new WorkerPort(
      this.store,
      namespace,
      resolved.artifact,
      resolved.admission?.tools,
    );
    const runtime: Runtime = {
      worker,
      account,
      observing: new Set(),
      inFlight: new Set(),
      controlBurst: 0,
      retryAfter: new Map(),
      abort: new AbortController(),
    };
    this.runtimes.set(key, runtime);
    worker.onFailure = () =>
      this.track(this.mailbox(namespace, () => this.unavailable(namespace)));
    try {
      requireValue(await worker.start(configuration, b));
      if (this.closing || b.signal.aborted)
        throw new HostFailure({ code: "unavailable", retry: "never" });
      const admitted = previous
        ? await VerifiedProviderSession.restore(
            worker,
            previous,
            configuration,
            b,
            resolved.admission,
          )
        : await VerifiedProviderSession.open(
            worker,
            configuration,
            b,
            resolved.admission,
          );
      const verified = requireValue(admitted);
      if (this.closing || b.signal.aborted)
        throw new HostFailure({ code: "unavailable", retry: "never" });
      let session: Session;
      if (previous)
        session = requireValue(
          await this.store.rebind({
            namespace,
            expectedRevision: previous.revision,
            expectedGeneration: previous.binding.generation,
            restored: verified,
            eventId: randomUUID(),
          }),
        );
      else {
        session = {
          schemaVersion: 2,
          kind: "session",
          namespace,
          revision: 0,
          lastSequence: 0,
          status: "active",
          binding: verified.binding,
          capabilities: verified.capabilities,
        };
        requireValue(await this.store.create(session));
      }
      runtime.verified = verified;
      worker.admitTools();
      worker.onFailure = () =>
        this.track(this.mailbox(namespace, () => this.unavailable(namespace)));
      if (previous) {
        await this.publishSince(namespace, previous.lastSequence);
        const snapshot = await this.snapshot(namespace);
        for (const record of snapshot.commands.filter(
          (row) => row.state === "reconciliation_required",
        ))
          this.track(this.reconcile(runtime, session, record));
      }
      queueMicrotask(() => this.kick(namespace));
      return ok(session);
    } catch (error) {
      this.diagnose("admission", error);
      const stopped = await worker.close(budget(2000));
      if (stopped.ok && stopped.value.processStopped) this.runtimes.delete(key);
      else this.blocked.add(key);
      throw error;
    }
  }
  createSession(
    caller: Caller,
    options: SessionOptions,
    b: Budget,
  ): Promise<Result<Session>> {
    if (this.closing || b.signal.aborted)
      return Promise.resolve(fail("unavailable"));
    const namespace = {
      tenantId: caller.tenantId,
      principalId: caller.principalId,
      authorityId: caller.authorityId,
      sessionId: randomUUID(),
    };
    return this.result(() =>
      this.admit(namespace, b, (admissionBudget) =>
        this.open(namespace, options, admissionBudget),
      ),
    );
  }
  resume(
    caller: Caller,
    sessionId: string,
    b: Budget,
  ): Promise<Result<Session>> {
    if (this.closing || b.signal.aborted)
      return Promise.resolve(fail("unavailable"));
    const namespace = {
      tenantId: caller.tenantId,
      principalId: caller.principalId,
      authorityId: caller.authorityId,
      sessionId,
    };
    return this.result(() =>
      this.admit(namespace, b, async (b) => {
        const previous = requireValue(await this.store.session(namespace));
        if (previous.status === "retired") return fail("session_gone");
        if (
          this.runtimes.get(namespaceKey(namespace))?.verified &&
          previous.status === "active"
        )
          return ok(previous);
        try {
          const result = await this.open(
            namespace,
            {
              provider: previous.binding.provider,
              config: previous.binding.config,
              accountRef: previous.binding.accountRef,
              profile:
                previous.capabilities.tools === "host_mediated"
                  ? "controlled_tools"
                  : "conversation",
            },
            b,
            previous,
          );
          if (!result.ok) await this.unavailable(namespace);
          return result;
        } catch (error) {
          await this.unavailable(namespace);
          throw error;
        }
      }),
    );
  }
  submit(caller: Caller, command: Command, b: Budget) {
    return command.input.type === "prompt"
      ? this.accept(caller, command, b)
      : Promise.resolve(fail<Receipt>("invalid_input"));
  }
  cancel(caller: Caller, command: Command, b: Budget) {
    return command.input.type === "cancel"
      ? this.accept(caller, command, b)
      : Promise.resolve(fail<Receipt>("invalid_input"));
  }
  respond(caller: Caller, command: Command, b: Budget) {
    return command.input.type === "respond"
      ? this.accept(caller, command, b)
      : Promise.resolve(fail<Receipt>("invalid_input"));
  }
  private accept(
    caller: Caller,
    command: Command,
    b: Budget,
    closing = false,
  ): Promise<Result<Receipt>> {
    if ((this.closing && !closing) || b.signal.aborted)
      return Promise.resolve(fail("unavailable"));
    return this.result(async () => {
      const decoded = decode(
        boundedJson(command, defaultLimits),
        defaultLimits,
      );
      if (decoded.kind !== "command") return fail("invalid_input");
      command = decoded;
      const namespace = {
        tenantId: caller.tenantId,
        principalId: caller.principalId,
        authorityId: caller.authorityId,
        sessionId: command.sessionId,
      };
      const result = await this.mailbox(namespace, async () => {
        const session = requireValue(await this.store.session(namespace)),
          prior = await this.store.command(namespace, command.commandId);
        if (!prior.ok) {
          if (
            session.status !== "active" ||
            !this.runtimes.get(namespaceKey(namespace))?.verified
          )
            return fail<Receipt>("reconciliation_required", "reconcile_first");
          const snapshot = await this.snapshot(namespace),
            input = command.input;
          const ordinaryInput =
            input.type === "prompt" && input.policy === "queue_next";
          if (
            snapshot.commands.filter(
              (row) =>
                row.state === "accepted" && isOrdinary(row) === ordinaryInput,
            ).length >= (ordinaryInput ? this.queueLimit : 64)
          )
            return fail<Receipt>("limit_exceeded");
          if (
            input.type === "prompt" &&
            input.policy === "steer" &&
            (session.capabilities.steer !== "supported" ||
              input.targetRunId !== session.binding.nativeRunId)
          )
            return fail<Receipt>("unsupported_capability");
          if (
            input.type !== "prompt" &&
            input.generation !== session.binding.generation
          )
            return fail<Receipt>("stale_binding");
          if (input.type === "cancel") {
            const target = snapshot.commands.find(
              (row) => row.command.commandId === input.targetCommandId,
            );
            if (!target || !isOrdinary(target))
              return fail<Receipt>("invalid_input");
            if (
              target.dispatch &&
              (input.nativeRunId !== target.dispatch.nativeRunId ||
                input.nativeRunId !== session.binding.nativeRunId)
            )
              return fail<Receipt>("stale_binding");
            if (
              target.dispatch &&
              ["unsupported", "unknown"].includes(
                session.capabilities.cancellation,
              )
            )
              return fail<Receipt>("unsupported_capability");
          }
        }
        const event = this.event(session, command.commandId, {
          type: "status",
          state: "accepted",
        });
        const receipt = await this.store.accept({
          namespace,
          command,
          expectedRevision: session.revision,
          expectedGeneration: session.binding.generation,
          nowMs: this.now(),
          retention: { retryWindowMs: 60000, receiptWindowMs: 86400000 },
          event,
        });
        if (receipt.ok)
          await this.publishSince(namespace, session.lastSequence);
        return receipt;
      });
      if (result.ok) this.kick(namespace);
      return result;
    });
  }
  private async snapshot(namespace: Namespace): Promise<{
    session: Session;
    commands: CommandRecord[];
    interactions: Interaction[];
    surfaces: SurfaceState[];
  }> {
    let page: SnapshotPage | undefined;
    const commands: CommandRecord[] = [],
      interactions: Interaction[] = [],
      surfaces: SurfaceState[] = [];
    do {
      page = requireValue(
        await this.store.snapshotPage(namespace, {
          limit: 256,
          ...(page?.next ? { continuation: page.next } : {}),
        }),
      );
      commands.push(...page.commands);
      interactions.push(...page.interactions);
      surfaces.push(...page.surfaces);
    } while (page.next);
    return { session: page.session, commands, interactions, surfaces };
  }
  private event(
    session: Session,
    commandId: string | undefined,
    body: Event["body"],
    attemptId?: string,
    offset = 0,
  ): Event {
    if (
      body.type === "invalidated" ||
      body.type === "cancelled" ||
      (body.type === "status" && body.state === "accepted")
    )
      attemptId = undefined;
    return {
      schemaVersion: 2,
      kind: "event",
      namespace: session.namespace,
      eventId: randomUUID(),
      sequence: session.lastSequence + offset + 1,
      generation: session.binding.generation,
      ...(commandId ? { commandId } : {}),
      ...(attemptId ? { attemptId } : {}),
      body,
    } as Event;
  }
  private async commit(
    session: Session,
    commands: CommandRecord[],
    events: Event[],
    extra: Partial<SessionCommit> = {},
  ) {
    const batch: SessionCommit = {
      namespace: session.namespace,
      expectedRevision: session.revision,
      expectedGeneration: session.binding.generation,
      session: {
        ...session,
        revision: session.revision + 1,
        lastSequence: session.lastSequence + events.length,
      },
      commands,
      events,
      interactions: [],
      surfaces: [],
      deliveries: [],
      nowMs: this.now(),
      ...extra,
    };
    requireValue(await this.store.commit(batch));
    for (const event of events)
      this.publish(session.namespace, { type: "event", event });
  }
  private kick(namespace: Namespace) {
    if (this.closed) return;
    this.track(
      this.mailbox(namespace, () => this.schedule(namespace)).catch(() =>
        this.mailbox(namespace, () => this.unavailable(namespace)),
      ),
    );
  }
  private async schedule(namespace: Namespace) {
    const runtime = this.runtimes.get(namespaceKey(namespace));
    if (!runtime?.verified || runtime.abort.signal.aborted) return;
    for (let n = 0; n < 16; n++) {
      const { session, commands } = await this.snapshot(namespace);
      if (session.status !== "active") return;
      const pending = commands
        .filter((row) => row.state === "accepted")
        .sort(
          (a, b) => a.receipt.acceptedRevision - b.receipt.acceptedRevision,
        );
      const head = pending.find(isOrdinary),
        ordinary =
          head &&
          (runtime.retryAfter.get(head.command.commandId) ?? 0) <= this.now()
            ? head
            : undefined,
        control = pending.find((row) => !isOrdinary(row));
      const occupied = commands.some(
        (row) => isOrdinary(row) && row.dispatch && !isSettled(row),
      );
      const record =
        control && (runtime.controlBurst < 8 || occupied || !ordinary)
          ? control
          : !occupied && !this.closing
            ? ordinary
            : control;
      if (!record || runtime.inFlight.size >= 16) return;
      if (isOrdinary(record)) {
        if (this.closing) return;
        runtime.controlBurst = 0;
      } else runtime.controlBurst++;
      if (record.command.expiresAtMs < this.now()) {
        const next: CommandRecord = {
          ...baseRecord(record),
          state: "invalidated",
          failure: { code: "expired", retry: "never" },
        };
        await this.commit(
          session,
          [next],
          [
            this.event(session, record.command.commandId, {
              type: "invalidated",
              failure: next.failure,
            }),
          ],
        );
        continue;
      }
      if (controlIsStale(session, commands, record.command)) {
        const next: CommandRecord = {
          ...baseRecord(record),
          state: "invalidated",
          failure: { code: "stale_binding", retry: "never" },
        };
        await this.commit(
          session,
          [next],
          [
            this.event(session, record.command.commandId, {
              type: "invalidated",
              failure: next.failure,
            }),
          ],
        );
        continue;
      }
      if (record.command.input.type === "cancel") {
        const target = commands.find(
          (row) =>
            row.command.commandId ===
            (
              record.command.input as Extract<
                Command["input"],
                { type: "cancel" }
              >
            ).targetCommandId,
        );
        if (target?.state === "accepted" && isOrdinary(target)) {
          const next: CommandRecord = {
            ...baseRecord(record),
            state: "acknowledged",
            acknowledgement: {
              type: "queued_cancelled",
              targetCommandId: target.command.commandId,
            },
          };
          const cancelled: CommandRecord = {
            ...baseRecord(target),
            state: "cancelled",
            cancelledBy: record.command.commandId,
          };
          await this.commit(
            session,
            [next, cancelled],
            [
              this.event(session, record.command.commandId, {
                type: "acknowledged",
                acknowledgement: next.acknowledgement,
              }),
              this.event(
                session,
                target.command.commandId,
                { type: "cancelled", cancelledBy: record.command.commandId },
                undefined,
                1,
              ),
            ],
          );
          continue;
        }
      }
      const attempt = {
        attemptId: randomUUID(),
        originGeneration: session.binding.generation,
        observerGeneration: session.binding.generation,
        nativeSessionId: session.binding.nativeSessionId,
        certainty: "intent" as const,
        ...(!isOrdinary(record)
          ? {
              ...(session.binding.nativeRunId
                ? { nativeRunId: session.binding.nativeRunId }
                : {}),
              ...(session.binding.nativeRequestId
                ? { nativeRequestId: session.binding.nativeRequestId }
                : {}),
            }
          : {}),
      };
      const next: CommandRecord = {
        ...baseRecord(record),
        state: "dispatching",
        dispatch: attempt,
      };
      await this.commit(
        session,
        [next],
        [
          this.event(
            session,
            record.command.commandId,
            { type: "dispatch", attempt },
            attempt.attemptId,
          ),
          this.event(
            session,
            record.command.commandId,
            { type: "status", state: "dispatching" },
            attempt.attemptId,
            1,
          ),
        ],
      );
      const current = requireValue(await this.store.session(namespace));
      runtime.inFlight.add(record.command.commandId);
      this.track(this.dispatch(runtime, current, next));
    }
    queueMicrotask(() => this.kick(namespace));
  }
  private async dispatch(
    runtime: Runtime,
    session: Session,
    record: CommandRecord,
  ) {
    try {
      const fact = requireValue(
        await runtime.verified!.dispatch(session, record, {
          timeoutMs: this.timeout,
          signal: runtime.abort.signal,
        }),
      );
      await this.mailbox(session.namespace, () =>
        this.applyFact(session.namespace, fact),
      );
      const current = await this.store.command(
        session.namespace,
        record.command.commandId,
      );
      if (
        current.ok &&
        isOrdinary(current.value) &&
        current.value.dispatch &&
        !isSettled(current.value)
      )
        this.observe(runtime, session.namespace, current.value);
    } catch (error) {
      this.diagnose("dispatch", error);
      await this.mailbox(session.namespace, () =>
        this.unavailable(session.namespace),
      );
    } finally {
      runtime.inFlight.delete(record.command.commandId);
      this.kick(session.namespace);
    }
  }
  private async reconcile(
    runtime: Runtime,
    session: Session,
    record: CommandRecord,
  ) {
    try {
      const fact = requireValue(
        await runtime.verified!.reconcile(session, record, {
          timeoutMs: this.timeout,
          signal: runtime.abort.signal,
        }),
      );
      await this.mailbox(session.namespace, () =>
        this.applyFact(session.namespace, fact),
      );
      const current = await this.store.command(
        session.namespace,
        record.command.commandId,
      );
      if (
        current.ok &&
        ["running", "dispatching"].includes(current.value.state)
      )
        this.observe(runtime, session.namespace, current.value);
    } catch (error) {
      this.diagnose("recovery", error);
      await this.mailbox(session.namespace, () =>
        this.unavailable(session.namespace),
      );
    } finally {
      this.kick(session.namespace);
    }
  }
  private observe(
    runtime: Runtime,
    namespace: Namespace,
    record: CommandRecord,
  ) {
    if (runtime.observing.has(record.command.commandId)) return;
    runtime.observing.add(record.command.commandId);
    this.track(
      (async () => {
        try {
          const session = requireValue(await this.store.session(namespace));
          for await (const fact of runtime.verified!.observe(session, record, {
            timeoutMs: 2147483647,
            signal: runtime.abort.signal,
          }))
            await this.mailbox(namespace, () =>
              this.applyFact(namespace, fact),
            );
          const current = await this.store.command(
            namespace,
            record.command.commandId,
          );
          if (
            current.ok &&
            !isSettled(current.value) &&
            !runtime.abort.signal.aborted
          )
            await this.mailbox(namespace, () => this.unavailable(namespace));
        } catch (error) {
          this.diagnose("observe", error);
          if (!runtime.abort.signal.aborted)
            await this.mailbox(namespace, () => this.unavailable(namespace));
        } finally {
          runtime.observing.delete(record.command.commandId);
          this.kick(namespace);
        }
      })(),
    );
  }
  private async applyFact(namespace: Namespace, proof: VerifiedProviderFact) {
    const snapshot = await this.snapshot(namespace),
      session = snapshot.session,
      record = snapshot.commands.find(
        (row) => row.command.commandId === proof.commandId,
      );
    if (session.status !== "active" || !record || isSettled(record)) return;
    const fact = providerFactFor(proof, session, record);
    if (!fact || !record.dispatch) return;
    if (
      (fact.status === "submitted" &&
        (record.state === "running" ||
          (record.state === "dispatching" &&
            record.dispatch.certainty === "submitted"))) ||
      (fact.status === "running" && record.state === "running")
    )
      return;
    const events: Event[] = [],
      interactions: Interaction[] = [],
      surfaces: SurfaceState[] = [];
    const append = (
      body: Event["body"],
      id = record.command.commandId,
      attempt = record.dispatch?.attemptId,
    ) => events.push(this.event(session, id, body, attempt, events.length));
    let next: CommandRecord = record;
    if (fact.status === "observed") {
      const observed = fact.observation;
      if (observed.type === "delta") {
        this.publish(namespace, {
          type: "delta",
          generation: session.binding.generation,
          commandId: record.command.commandId,
          messageId: observed.messageId,
          text: observed.text,
        });
        return;
      }
      if (observed.type === "event") {
        if (observed.body.type === "surface") {
          surfaces.push(observed.body.surface);
        }
        append(observed.body);
      } else if (observed.type === "interaction") {
        const row: Interaction = {
          schemaVersion: 2,
          kind: "interaction",
          namespace,
          commandId: record.command.commandId,
          generation: session.binding.generation,
          ...(fact.binding.nativeRunId
            ? { nativeRunId: fact.binding.nativeRunId }
            : {}),
          ...observed.interaction,
          status: "pending",
        };
        interactions.push(row);
        append({
          type: "interaction",
          interactionId: row.interactionId,
          status: "pending",
          request: row.request,
          expiresAtMs: row.expiresAtMs,
          callbackLifetime: row.callbackLifetime,
        });
      } else if (observed.type === "interaction_unavailable") {
        const row = snapshot.interactions.find(
          (row) => row.interactionId === observed.interactionId,
        );
        if (!row || row.status !== "pending") return;
        interactions.push({ ...row, status: "unavailable" });
        append({
          type: "interaction",
          interactionId: row.interactionId,
          status: "unavailable",
        });
      } else return;
    } else {
      append({
        type: "reconciled",
        attempt: record.dispatch,
        resolution: fact.status,
      });
      if (fact.status === "not_submitted") {
        const retry =
          isOrdinary(record) &&
          this.now() <=
            Math.min(record.command.expiresAtMs, record.receipt.retryUntilMs) &&
          fact.error?.retry !== "never";
        next = retry
          ? { ...baseRecord(record), state: "accepted" }
          : {
              ...baseRecord(record),
              state: "invalidated",
              failure: {
                code:
                  this.now() > record.command.expiresAtMs
                    ? "expired"
                    : "unavailable",
                retry: "never",
              },
            };
        // The transition itself owns retry eligibility, including the original receipt window.
        if (next.state === "accepted")
          append(
            { type: "status", state: "accepted" },
            record.command.commandId,
            undefined,
          );
        else
          append(
            { type: "invalidated", failure: next.failure },
            record.command.commandId,
            undefined,
          );
      } else {
        const dispatch = {
          ...record.dispatch,
          ...(fact.binding.nativeRunId
            ? { nativeRunId: fact.binding.nativeRunId }
            : {}),
          ...(fact.binding.nativeRequestId
            ? { nativeRequestId: fact.binding.nativeRequestId }
            : {}),
          certainty:
            fact.status === "unknown"
              ? record.dispatch.certainty === "submitted"
                ? ("submitted" as const)
                : ("unknown" as const)
              : ("submitted" as const),
          ...(fact.status === "unknown" &&
          record.dispatch.certainty !== "submitted"
            ? {
                correlationId:
                  record.dispatch.correlationId ??
                  (fact.status === "unknown"
                    ? fact.correlationId
                    : undefined) ??
                  record.dispatch.attemptId,
              }
            : {}),
        };
        if (!same(dispatch, record.dispatch))
          append({ type: "dispatch", attempt: dispatch });
        if (fact.status === "terminal")
          next = {
            ...baseRecord(record),
            dispatch,
            state: "terminal",
            outcome: fact.outcome,
          };
        else if (fact.status === "acknowledged")
          next = {
            ...baseRecord(record),
            dispatch,
            state: "acknowledged",
            acknowledgement: fact.acknowledgement,
          };
        else
          next = {
            ...baseRecord(record),
            dispatch,
            state:
              fact.status === "unknown"
                ? "reconciliation_required"
                : fact.status === "submitted"
                  ? "dispatching"
                  : "running",
          };
        if (next.state === "terminal")
          append({ type: "terminal", outcome: next.outcome });
        else if (
          next.state === "acknowledged" &&
          fact.status === "acknowledged"
        )
          append({
            type: "acknowledged",
            acknowledgement: fact.acknowledgement,
          });
        else if (
          next.state !== record.state &&
          (next.state === "dispatching" ||
            next.state === "running" ||
            next.state === "reconciliation_required")
        )
          append({ type: "status", state: next.state });
      }
      if (isSettled(next)) {
        for (const row of snapshot.interactions.filter(
          (row) =>
            row.commandId === record.command.commandId &&
            row.status === "pending",
        )) {
          interactions.push({ ...row, status: "unavailable" });
          append({
            type: "interaction",
            interactionId: row.interactionId,
            status: "unavailable",
          });
        }
        const ids = new Set(
          snapshot.interactions
            .filter((row) => row.commandId === record.command.commandId)
            .map((row) => row.interactionId),
        );
        for (const row of snapshot.surfaces.filter(
          (row) => ids.has(row.interactionId) && row.status === "active",
        )) {
          const invalidated = {
            ...row,
            status: "invalidated" as const,
            revision: row.revision + 1,
          };
          surfaces.push(invalidated);
          append({ type: "surface", surface: invalidated });
        }
      }
    }
    const binding =
      fact.status === "submitted" ||
      fact.status === "running" ||
      fact.status === "terminal"
        ? fact.binding
        : session.binding;
    await this.commit(session, [next], events, {
      providerFacts: [proof],
      interactions,
      surfaces,
      session: {
        ...session,
        binding,
        revision: session.revision + 1,
        lastSequence: session.lastSequence + events.length,
      },
    });
    if (fact.status === "not_submitted" && next.state === "accepted") {
      this.runtimes
        .get(namespaceKey(namespace))
        ?.retryAfter.set(next.command.commandId, this.now() + 100);
      const timer = setTimeout(() => {
        this.retryTimers.delete(timer);
        this.kick(namespace);
      }, 100);
      this.retryTimers.add(timer);
    } else this.kick(namespace);
  }
  private async unavailable(namespace: Namespace) {
    const session = await this.store.session(namespace);
    if (!session.ok || session.value.status === "retired") return;
    if (session.value.status !== "recovery_required") {
      const s = session.value;
      requireValue(
        await this.store.recoverUnavailable({
          namespace,
          expectedRevision: s.revision,
          expectedGeneration: s.binding.generation,
          eventId: randomUUID(),
        }),
      );
      await this.publishSince(namespace, s.lastSequence);
    }
    const key = namespaceKey(namespace),
      runtime = this.runtimes.get(key);
    if (runtime) {
      runtime.abort.abort();
      runtime.worker.onFailure = undefined;
      this.track(
        runtime.worker.close(budget(2000)).then((result) => {
          if (result.ok && result.value.processStopped)
            this.runtimes.delete(key);
          else this.blocked.add(key);
        }),
      );
    }
  }
  private publish(namespace: Namespace, item: Subscription) {
    for (const stream of this.subscribers.get(namespaceKey(namespace)) ?? []) {
      if (!stream.push(item)) stream.end({ type: "resync_required" });
    }
  }
  private async publishSince(namespace: Namespace, after: number) {
    for (;;) {
      const events = requireValue(
        await this.store.events(namespace, after, 256),
      );
      for (const event of events)
        this.publish(namespace, { type: "event", event });
      if (events.length < 256) return;
      after = events.at(-1)!.sequence;
    }
  }
  surface(caller: Caller, sessionId: string, instanceId: string, b: Budget) {
    return this.closed || b.signal.aborted
      ? Promise.resolve(fail<SurfaceState>("unavailable"))
      : this.store.surface({ ...caller, sessionId }, instanceId);
  }
  snapshotPage(caller: Caller, sessionId: string, query: PageQuery, b: Budget) {
    return this.closed || b.signal.aborted
      ? Promise.resolve(fail<SnapshotPage>("unavailable"))
      : this.store.snapshotPage({ ...caller, sessionId }, query);
  }
  listSessions(caller: Caller, query: PageQuery, b: Budget) {
    return this.closed || b.signal.aborted
      ? Promise.resolve(
          fail<import("@rss-mdm-agent/ai-contract").SessionPage>("unavailable"),
        )
      : this.store.listSessions(caller, query);
  }
  async *subscribe(
    caller: Caller,
    sessionId: string,
    after: number,
    b: Budget,
  ): AsyncIterable<Subscription> {
    const namespace = { ...caller, sessionId },
      key = namespaceKey(namespace),
      stream = new Output<Subscription>();
    const abort = () => stream.end();
    b.signal.addEventListener("abort", abort, { once: true });
    try {
      await this.mailbox(namespace, async () => {
        const session = requireValue(await this.store.session(namespace));
        if (
          this.closed ||
          b.signal.aborted ||
          !Number.isSafeInteger(after) ||
          after < 0 ||
          after > session.lastSequence
        ) {
          stream.end({ type: "resync_required" });
          return;
        }
        const listeners = this.subscribers.get(key) ?? new Set();
        if (listeners.size >= 64) {
          stream.end({ type: "resync_required" });
          return;
        }
        listeners.add(stream);
        this.subscribers.set(key, listeners);
        const events = requireValue(
          await this.store.events(namespace, after, 256),
        );
        if (session.lastSequence - after > events.length) {
          stream.end({ type: "resync_required" });
          return;
        }
        for (const event of events)
          if (!stream.push({ type: "event", event })) {
            stream.end({ type: "resync_required" });
            break;
          }
      });
      for await (const item of stream) yield item;
    } catch {
      yield { type: "resync_required" };
    } finally {
      stream.end();
      b.signal.removeEventListener("abort", abort);
      const listeners = this.subscribers.get(key);
      listeners?.delete(stream);
      if (!listeners?.size) this.subscribers.delete(key);
    }
  }
  close(b: Budget): Promise<Result<void>> {
    if (this.closingTask) return this.closingTask;
    const task = this.result(() => this.stop(b));
    this.closingTask = task;
    void task
      .finally(() => {
        if (this.closingTask === task) this.closingTask = undefined;
      })
      .catch(() => {});
    return task;
  }
  private async stop(b: Budget): Promise<Result<void>> {
    if (this.closed) return ok(undefined);
    this.closing = true;
    const deadline = Date.now() + b.timeoutMs;
    for (const abort of this.admissions.values()) abort.abort();
    if (this.admissions.size) {
      let timer: ReturnType<typeof setTimeout> | undefined;
      const drained = await Promise.race([
        Promise.allSettled([...this.admissions.keys()]).then(() => true),
        new Promise<false>((resolve) => {
          timer = setTimeout(
            () => resolve(false),
            Math.max(1, deadline - Date.now()),
          );
        }),
      ]).finally(() => clearTimeout(timer));
      if (!drained) return fail("unavailable", "same_command");
    }
    for (const timer of this.retryTimers) clearTimeout(timer);
    this.retryTimers.clear();
    for (const [key, runtime] of this.runtimes) {
      const namespace = JSON.parse(key) as string[],
        ns = {
          tenantId: namespace[0],
          principalId: namespace[1],
          authorityId: namespace[2],
          sessionId: namespace[3],
        };
      const snapshot = await this.snapshot(ns);
      for (const record of snapshot.commands.filter(
        (row) => isOrdinary(row) && row.dispatch && !isSettled(row),
      )) {
        await this.accept(
          ns,
          {
            schemaVersion: 2,
            kind: "command",
            sessionId: ns.sessionId,
            commandId: randomUUID(),
            expiresAtMs: this.now() + Math.max(1, deadline - Date.now()),
            input: {
              type: "cancel",
              targetCommandId: record.command.commandId,
              generation: snapshot.session.binding.generation,
              ...(record.dispatch?.nativeRunId
                ? { nativeRunId: record.dispatch.nativeRunId }
                : {}),
            },
          },
          b,
          true,
        );
      }
      runtime.worker.onFailure = undefined;
    }
    // Allow cancellation observations to settle without admitting the next queued turn.
    const settleUntil = Math.min(deadline - 1000, Date.now() + 1000);
    while (this.tasks.size && Date.now() < settleUntil && !b.signal.aborted)
      await new Promise((resolve) => setTimeout(resolve, 10));
    let stopped = true;
    for (const [key, runtime] of [...this.runtimes]) {
      runtime.abort.abort();
      const result = await runtime.worker.close({
        timeoutMs: Math.max(1, deadline - Date.now()),
        signal: b.signal,
      });
      if (!result.ok || !result.value.processStopped) {
        this.diagnose(
          "close",
          !result.ok ? new HostFailure(result.error) : undefined,
        );
        stopped = false;
        continue;
      }
      const [tenantId, principalId, authorityId, sessionId] = JSON.parse(key);
      const namespace = { tenantId, principalId, authorityId, sessionId };
      await this.mailbox(namespace, () => this.unavailable(namespace));
      this.runtimes.delete(key);
    }
    for (const listeners of this.subscribers.values())
      for (const stream of listeners) stream.end();
    this.subscribers.clear();
    if (!stopped) return fail("unavailable", "same_command");
    const result = await this.store.close({
      timeoutMs: Math.max(1, deadline - Date.now()),
      signal: b.signal,
    });
    if (result.ok) this.closed = true;
    return result;
  }
}
export async function createHost(
  options: HostOptions,
): Promise<Result<SessionHost>> {
  return SessionHost.create(options);
}
