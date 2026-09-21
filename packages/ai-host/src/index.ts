import type { PreferencesPatch } from "@rss-mdm-agent/ai-contract";
import { isDeepStrictEqual } from "node:util";
import {
  productSession,
  replaceStage,
  historyPreview,
  type HistoryPreview,
  type ConnectionPage,
  type UserPreferences,
  connectionRevision,
} from "@rss-mdm-agent/ai-contract";
import { activeStage } from "@rss-mdm-agent/ai-contract";
import { randomUUID } from "node:crypto";
import {
  boundedJson,
  decode,
  interactionCatalog,
  type Budget,
  type Binding,
  type ToolEndpoint,
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
  type ConnectionOptions,
  type SessionOptions,
  type Connection,
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
import { Deliveries, type DeliveryRouter } from "./delivery.js";
export type {
  DeliveryRouter,
  DeliveryRequest,
  DeliveryReceipt,
} from "./delivery.js";
import { WorkerPort, groupEmpty } from "./process.js";
import { Output } from "./queue.js";
import { Deadline } from "./deadline.js";
import type { WorkerLaunchFenceStore } from "./launch-fence.js";

export interface HostDiagnostic {
  readonly stage:
    | "admission"
    | "dispatch"
    | "observe"
    | "recovery"
    | "credential"
    | "close";
  readonly code: import("@rss-mdm-agent/ai-contract").Failure["code"];
}
export interface HostOptions {
  /** Trusted persistence composition; secrets never enter public wire records. */
  readonly persistConnection?: (
    caller: Caller,
    connection: Connection,
    expected: number | null,
    secret: string | undefined,
    budget: Budget,
  ) => Promise<Result<Connection>>;
  readonly callerAvailable?: (caller: Caller) => boolean;
  readonly onDiagnostic?: (diagnostic: HostDiagnostic) => void;
  readonly store: SessionStore;
  readonly launchFences: WorkerLaunchFenceStore;
  readonly delivery: DeliveryRouter | null;
  /** Trusted composition resolves metadata and memory-only worker activation. */
  resolve(
    caller: Caller,
    options: ConnectionOptions,
    namespace: Namespace,
    budget: Budget,
    previous: Binding | null,
    candidate?: Connection,
    secret?: string,
  ): Promise<{
    configuration: ProviderConfiguration;
    artifact: string;
    activation?: unknown;
    dispose?: () => Promise<void>;
    admission?: Pick<ProviderAdmission, "verifier">;
  }>;
  readonly queueLimit?: number;
  readonly workerLimit?: number;
  readonly operationTimeoutMs?: number;
  readonly now?: () => number;
}
interface Runtime {
  verification?: true;
  dispose?: () => Promise<void>;
  worker: WorkerPort;
  verified?: VerifiedProviderSession;
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
  schemaVersion: 5 as const,
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
/** Closed expected host failure for trusted composition adapters. */
export class HostFailure extends Error {
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
  private readonly suspended = new Set<string>();
  private readonly disposals = new Set<() => Promise<void>>();
  private async saveValidatedConnection(
    caller: Caller,
    connection: Connection,
    expected: number | null,
    b: Budget,
    secret?: string,
  ): Promise<Result<Connection>> {
    if (!this.callerAvailable(caller) || b.signal.aborted)
      return Promise.resolve(fail("unavailable"));
    try {
      const result = this.options.persistConnection
        ? await this.options.persistConnection(
            caller,
            connection,
            expected,
            secret,
            b,
          )
        : await this.store.saveConnection(caller, connection, expected);
      if (!result.ok)
        this.diagnose("credential", new HostFailure(result.error));
      return result;
    } catch (error) {
      this.diagnose("credential", error);
      throw error;
    }
  }
  private callerKey(caller: Caller) {
    return JSON.stringify([
      caller.tenantId,
      caller.principalId,
      caller.authorityId,
    ]);
  }
  private callerAvailable(caller: Caller) {
    return (
      !this.suspended.has(this.callerKey(caller)) &&
      (this.options.callerAvailable?.(caller) ?? true)
    );
  }
  private readonly admissions = new Map<
    Promise<unknown>,
    { abort: AbortController; caller: string }
  >();
  private closingTask?: Promise<Result<void>>;
  private closing = false;
  private closed = false;
  private readonly now: () => number;
  private readonly queueLimit: number;
  private readonly workerLimit: number;
  private readonly timeout: number;
  private readonly deliveries?: Deliveries;
  private readonly deliveryAbort = new AbortController();
  private deliveryFailure?: string;
  private constructor(private readonly options: HostOptions) {
    this.now = options.now ?? Date.now;
    this.queueLimit = options.queueLimit ?? 64;
    this.workerLimit = options.workerLimit ?? 8;
    this.timeout = options.operationTimeoutMs ?? 30000;
    if (options.delivery)
      this.deliveries = new Deliveries(
        options.store,
        options.delivery,
        (n, fn) => this.mailbox(n, fn),
        (n, after) => this.publishSince(n, after),
        this.now,
      );
  }
  static async create(options: HostOptions): Promise<Result<SessionHost>> {
    if (
      [
        options.queueLimit ?? 64,
        options.workerLimit ?? 8,
        options.operationTimeoutMs ?? 30000,
      ].some((n) => !Number.isSafeInteger(n) || n < 1 || n > 2147483647)
    )
      return fail("invalid_input");
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
  private ownDispose(dispose: (() => Promise<void>) | undefined) {
    if (dispose) this.disposals.add(dispose);
    return dispose;
  }
  private async releaseDispose(
    dispose: (() => Promise<void>) | undefined,
  ): Promise<boolean> {
    if (!dispose) return true;
    try {
      await dispose();
      this.disposals.delete(dispose);
      return true;
    } catch (error) {
      this.diagnose("close", error);
      return false;
    }
  }
  private async releaseRuntime(
    key: string,
    runtime: Runtime,
  ): Promise<boolean> {
    if (this.runtimes.get(key) === runtime) this.runtimes.delete(key);
    return this.releaseDispose(runtime.dispose);
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
    void task
      .catch((error) => this.diagnose("recovery", error))
      .finally(() => this.tasks.delete(task));
  }
  private admit<T>(
    namespace: Namespace,
    b: Budget,
    action: (budget: Budget) => Promise<Result<T>>,
    allowFenced = false,
  ): Promise<Result<T>> {
    const abort = new AbortController();
    const task = this.mailbox(namespace, () =>
      b.signal.aborted ||
      abort.signal.aborted ||
      (!allowFenced && (this.closing || !this.callerAvailable(namespace)))
        ? Promise.resolve(fail<T>("unavailable"))
        : action({ ...b, signal: AbortSignal.any([b.signal, abort.signal]) }),
    );
    this.admissions.set(task, { abort, caller: this.callerKey(namespace) });
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
    if (offered.contractVersion !== 5 || offered.acp !== 1)
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
      for (const launch of requireValue(
        await this.options.launchFences.launches(),
      )) {
        if (launch.phase === "reserved" || groupEmpty(launch.pgid))
          requireValue(
            await this.options.launchFences.releaseLaunch(
              launch.namespace,
              launch.launchId,
            ),
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
      for (const namespace of namespaces.values()) {
        if (this.callerAvailable(namespace))
          await this.resume(
            namespace,
            namespace.sessionId,
            budget(this.timeout),
          );
        else await this.unavailable(namespace);
      }
      this.sweepDeliveries();
      return ok(undefined);
    });
  }
  private sweepDeliveries() {
    if (!this.deliveries || this.closing) return;
    this.track(
      this.deliveries
        .recover({ timeoutMs: this.timeout, signal: this.deliveryAbort.signal })
        .then((result) => {
          if (result.ok) this.deliveryFailure = undefined;
          else if (result.error.code !== this.deliveryFailure) {
            this.deliveryFailure = result.error.code;
            this.diagnose("recovery", new HostFailure(result.error));
          }
        })
        .finally(() => {
          if (this.closing) return;
          const timer = setTimeout(() => {
            this.retryTimers.delete(timer);
            this.sweepDeliveries();
          }, 1000);
          timer.unref();
          this.retryTimers.add(timer);
        }),
    );
  }
  private async propose(
    namespace: Namespace,
    proposal: Parameters<ToolEndpoint["propose"]>[0],
    b: Budget,
  ): ReturnType<ToolEndpoint["propose"]> {
    if (this.closing || !this.deliveries) return fail("unavailable");
    const runtime = this.runtimes.get(namespaceKey(namespace));
    if (!runtime?.verified || runtime.abort.signal.aborted)
      return fail("stale_binding");
    const generation = runtime.verified.binding.generation;
    const snapshot = await this.snapshot(namespace);
    const active = snapshot.commands.filter(
      (c) =>
        c.command.input.type === "prompt" &&
        (c.state === "running" || c.state === "dispatching") &&
        c.dispatch?.observerGeneration === generation,
    );
    if (active.length !== 1) return fail("stale_binding");
    return this.deliveries.propose(
      namespace,
      generation,
      active[0]!.command.commandId,
      proposal,
      b,
    );
  }
  private async open(
    namespace: Namespace,
    options: ConnectionOptions,
    b: Budget,
    session: Session,
    connection: Connection,
    restore: boolean,
  ): Promise<Result<Session>> {
    const previous = restore ? session : undefined;
    if (this.closing || b.signal.aborted) return fail("unavailable");
    // Retry orphan collection on admission, without a heartbeat or signaling old PIDs.
    for (const launch of requireValue(
      await this.options.launchFences.launches(),
    )) {
      const blockedKey = namespaceKey(launch.namespace);
      if (
        this.blocked.has(blockedKey) &&
        (launch.phase === "reserved" || groupEmpty(launch.pgid))
      ) {
        requireValue(
          await this.options.launchFences.releaseLaunch(
            launch.namespace,
            launch.launchId,
          ),
        );
        this.blocked.delete(blockedKey);
      }
    }
    const key = namespaceKey(namespace);
    const incomplete = this.runtimes.get(key);
    if (incomplete && incomplete.abort.signal.aborted && !incomplete.verified) {
      const stopped = await incomplete.worker.close(b);
      if (
        stopped.ok &&
        stopped.value.processStopped &&
        (await this.releaseRuntime(key, incomplete))
      )
        this.blocked.delete(key);
    }
    if (this.runtimes.has(key))
      return fail("reconciliation_required", "reconcile_first");
    if (this.blocked.has(key)) {
      const fence = requireValue(
        await this.options.launchFences.launches(),
      ).find((row) => namespaceKey(row.namespace) === key);
      if (fence?.phase === "registered" && !groupEmpty(fence.pgid))
        return fail("reconciliation_required", "reconcile_first");
      if (fence)
        requireValue(
          await this.options.launchFences.releaseLaunch(
            namespace,
            fence.launchId,
          ),
        );
      this.blocked.delete(key);
    }
    const resolved = await this.options.resolve(
        namespace,
        options,
        namespace,
        b,
        previous?.currentStageId ? activeStage(previous).binding : null,
      ),
      dispose = this.ownDispose(resolved.dispose),
      configuration = structuredClone(resolved.configuration);
    if (this.closing || b.signal.aborted) {
      await this.releaseDispose(dispose);
      return fail("unavailable");
    }
    if (
      namespaceKey(configuration.namespace) !== namespaceKey(namespace) ||
      configuration.provider !== options.provider ||
      configuration.config.id !== options.config.id ||
      configuration.config.revision !== options.config.revision ||
      configuration.permissions !==
        (options.profile === "controlled_tools"
          ? "host_mediated"
          : "tools_disabled")
    ) {
      await this.releaseDispose(dispose);
      return fail("permission_denied");
    }
    if (this.runtimes.size >= this.workerLimit) {
      await this.releaseDispose(dispose);
      return fail("limit_exceeded");
    }
    if (
      options.profile === "controlled_tools" &&
      (!resolved.admission || !this.deliveries)
    ) {
      await this.releaseDispose(dispose);
      return fail("unsupported_capability");
    }
    const tools: ToolEndpoint = {
      propose: (proposal, b) => this.propose(namespace, proposal, b),
    };
    const admission = resolved.admission
      ? { verifier: resolved.admission.verifier, tools }
      : undefined;
    const worker = new WorkerPort(
      this.options.launchFences,
      namespace,
      resolved.artifact,
      admission?.tools,
      resolved.activation,
    );
    const runtime: Runtime = {
      dispose,
      worker,
      observing: new Set(),
      inFlight: new Set(),
      controlBurst: 0,
      retryAfter: new Map(),
      abort: new AbortController(),
    };
    this.runtimes.set(key, runtime);
    worker.onFailure = () => {
      this.isolate(namespace);
      this.track(this.mailbox(namespace, () => this.unavailable(namespace)));
    };
    try {
      requireValue(
        await worker.start(
          configuration,
          b,
          previous?.currentStageId ? activeStage(previous).binding : null,
        ),
      );
      if (this.closing || b.signal.aborted)
        throw new HostFailure({ code: "unavailable", retry: "never" });
      const admitted = previous
        ? await VerifiedProviderSession.restore(
            worker,
            previous,
            configuration,
            b,
            admission,
          )
        : await VerifiedProviderSession.open(
            worker,
            configuration,
            b,
            admission,
          );
      const verified = requireValue(admitted);
      if (this.closing || b.signal.aborted)
        throw new HostFailure({ code: "unavailable", retry: "never" });
      if (previous)
        session = requireValue(
          await this.store.rebind({
            namespace,
            expectedRevision: previous.revision,
            expectedGeneration: activeStage(previous).binding.generation,
            restored: verified,
            eventId: randomUUID(),
          }),
        );
      else {
        session = requireValue(
          await this.store.activateStage({
            namespace,
            expectedRevision: session.revision,
            configRevision: connection.configRevision,

            opened: verified,
          }),
        );
      }
      runtime.verified = verified;
      this.publish(namespace, { type: "resync_required" });
      worker.admitTools();
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
      runtime.abort.abort();
      const stopped = await worker.close(budget(2000));
      if (
        !stopped.ok ||
        !stopped.value.processStopped ||
        !(await this.releaseRuntime(key, runtime))
      )
        this.blocked.add(key);
      throw error;
    }
  }
  connections(caller: Caller, b: Budget): Promise<Result<ConnectionPage>> {
    return this.result(async () => {
      if (b.signal.aborted || this.closing || !this.callerAvailable(caller))
        return fail("unavailable");
      return ok({
        schemaVersion: 5,
        kind: "connectionPage",
        connections: [...requireValue(await this.store.connections(caller))],
        preferences: requireValue(await this.store.preferences(caller)),
      });
    });
  }
  saveConnection(
    caller: Caller,
    connection: Connection,
    expected: number | null,
    b: Budget,
    secret?: string,
  ): Promise<Result<Connection>> {
    const namespace = {
      ...caller,
      sessionId: `connection-${connection.connectionId}`,
    };
    return this.result(() =>
      this.admit(namespace, b, async (b) => {
        if (!this.callerAvailable(caller)) return fail("unavailable");
        const old = await this.store.connection(
          caller,
          connection.connectionId,
        );
        const checked = connectionRevision(
          connection,
          old.ok ? old.value : undefined,
          expected,
        );
        if (!checked.ok) return checked;
        if (connection.status === "deleted")
          return this.saveValidatedConnection(
            caller,
            connection,
            expected,
            b,
            secret,
          );
        if (this.runtimes.size >= this.workerLimit)
          return fail("limit_exceeded");
        const probe = {
          ...namespace,
          sessionId: `verification-${randomUUID()}`,
        };
        const resolved = await this.options.resolve(
          caller,
          this.connectionOptions(connection),
          probe,
          b,
          null,
          connection,
          secret,
        );
        const dispose = this.ownDispose(resolved.dispose);
        let probing = false,
          toolObserved = false,
          toolViolation = false;
        const tools: ToolEndpoint = {
          propose: async (proposal) => {
            if (
              !probing ||
              toolObserved ||
              proposal.name !== "connection_probe" ||
              Object.keys(proposal.arguments).length !== 0
            ) {
              toolViolation = true;
              return fail("permission_denied");
            }
            toolObserved = true;
            return ok({
              disposition: "returned",
              text: "Connection probe passed. No device operation was performed.",
            });
          },
        };
        const worker = new WorkerPort(
          this.options.launchFences,
          probe,
          resolved.artifact,
          resolved.admission ? tools : undefined,
          resolved.activation,
        );
        const runtime: Runtime = {
          verification: true,
          dispose,
          worker,
          observing: new Set(),
          inFlight: new Set(),
          controlBurst: 0,
          retryAfter: new Map(),
          abort: new AbortController(),
        };
        const key = namespaceKey(probe);
        this.runtimes.set(key, runtime);
        b = { ...b, signal: AbortSignal.any([b.signal, runtime.abort.signal]) };
        try {
          requireValue(await worker.start(resolved.configuration, b, null));
          const verified = requireValue(
            await VerifiedProviderSession.open(
              worker,
              resolved.configuration,
              b,
              resolved.admission
                ? { verifier: resolved.admission.verifier, tools }
                : undefined,
            ),
          );
          const command: Command = {
            schemaVersion: 5,
            kind: "command",
            sessionId: probe.sessionId,
            commandId: randomUUID(),
            expiresAtMs: this.now() + b.timeoutMs,
            input: {
              type: "prompt",
              policy: "queue_next",
              text:
                connection.profile === "controlled_tools"
                  ? 'Call rss_host.propose once with name "connection_probe" and arguments {}. Then reply OK. This verifies the connection only; do not request any device operation.'
                  : "Reply with OK only. Do not use any tools.",
            },
          };
          const binding = verified.binding;
          // This verification bridge exposes only the harmless probe, never device execution.
          worker.admitTools();
          probing = true;
          const sent = await worker.dispatch(
            binding,
            command,
            {
              attemptId: randomUUID(),
              originGeneration: binding.generation,
              observerGeneration: binding.generation,
              nativeSessionId: binding.nativeSessionId,
              ...(binding.nativeThreadId
                ? { nativeThreadId: binding.nativeThreadId }
                : {}),
              certainty: "intent",
            },
            b,
          );
          if (sent.certainty === "not_sent")
            return { ok: false, error: sent.error };
          if (sent.certainty !== "submitted") return fail("unavailable");
          let completed = false;
          let failure: import("@rss-mdm-agent/ai-contract").Failure = {
            code: "unavailable",
            retry: "never",
          };
          for await (const item of worker.observe(sent.binding, b)) {
            if (item.type !== "event") continue;
            if (item.body.type === "error") failure = item.body.failure;
            if (item.body.type === "terminal") {
              completed = item.body.outcome === "completed";
              if (item.body.outcome === "cancelled")
                failure = { code: "verification_cancelled", retry: "never" };
              if (item.body.outcome === "refused")
                failure = { code: "verification_refused", retry: "never" };
              if (
                ["max_tokens", "max_turn_requests"].includes(item.body.outcome)
              )
                failure = { code: "limit_exceeded", retry: "never" };
              break;
            }
          }
          probing = false;
          if (!completed) return { ok: false, error: failure };
          if (
            connection.profile === "controlled_tools" &&
            (!toolObserved || toolViolation)
          )
            return fail("unsupported_capability");
          const stopped = requireValue(await worker.close(b));
          if (
            !stopped.processStopped ||
            b.signal.aborted ||
            !this.callerAvailable(caller)
          )
            return fail("unavailable");
          return await this.saveValidatedConnection(
            caller,
            { ...connection, status: "ready" },
            expected,
            b,
            secret,
          );
        } finally {
          const stopped = await worker.close(budget(2000));
          if (stopped.ok && stopped.value.processStopped) {
            await this.releaseRuntime(key, runtime);
          } else {
            runtime.abort.abort();
            worker.terminate();
            this.blocked.add(key);
          }
        }
      }),
    );
  }
  savePreferences(
    caller: Caller,
    preferences: PreferencesPatch,
    b: Budget,
  ): Promise<Result<UserPreferences>> {
    return this.result(() =>
      this.admit({ ...caller, sessionId: "preferences" }, b, async () =>
        this.store.savePreferences(caller, preferences),
      ),
    );
  }
  previewHistory(
    caller: Caller,
    sessionId: string,
    connectionId: string,
    recent: number | undefined,
    b: Budget,
  ): Promise<Result<HistoryPreview>> {
    return this.result(async () => {
      if (b.signal.aborted || this.closing || !this.callerAvailable(caller))
        return fail("unavailable");
      const namespace = { ...caller, sessionId };
      const connection = requireValue(
        await this.store.connection(caller, connectionId),
      );
      if (connection.status !== "ready") return fail("connection_required");
      const snapshot = await this.snapshot(namespace),
        events: Event[] = [];
      let after = 0;
      while (after < snapshot.session.lastSequence) {
        const rows = requireValue(
          await this.store.events(namespace, after, 256),
        );
        if (!rows.length) return fail("storage_corrupt");
        events.push(
          ...rows.filter(
            (row) => row.sequence <= snapshot.session.lastSequence,
          ),
        );
        after = rows.at(-1)!.sequence;
      }
      return historyPreview(
        snapshot.session,
        snapshot.commands,
        events,
        connection,
        recent,
      );
    });
  }
  private async checkHistory(
    session: Session,
    preview: HistoryPreview,
  ): Promise<Result<void>> {
    if (
      preview.sessionId !== session.namespace.sessionId ||
      preview.connectionId !== session.selectedConnectionId ||
      preview.throughSequence > session.lastSequence
    )
      return fail("content_conflict");
    const connection = requireValue(
      await this.store.connection(session.namespace, preview.connectionId),
    );
    if (connection.configRevision !== preview.configRevision)
      return fail("content_conflict");
    if (
      !session.freshContext &&
      session.currentStageId &&
      activeStage(session).connectionId === connection.connectionId &&
      activeStage(session).configRevision === connection.configRevision
    )
      return fail("context_unavailable");
    const snapshot = await this.snapshot(session.namespace),
      events: Event[] = [];
    let after = 0;
    while (after < preview.throughSequence) {
      const rows = requireValue(
        await this.store.events(session.namespace, after, 256),
      );
      if (!rows.length) return fail("storage_corrupt");
      events.push(
        ...rows.filter((row) => row.sequence <= preview.throughSequence),
      );
      after = rows.at(-1)!.sequence;
    }
    const completed = new Set(
      events
        .filter(
          (row) =>
            row.body.type === "terminal" && row.body.outcome === "completed",
        )
        .map((row) => row.commandId),
    );
    if (preview.commandIds.some((id) => !completed.has(id)))
      return fail("content_conflict");
    const expected = historyPreview(
      { ...session, lastSequence: preview.throughSequence },
      snapshot.commands.filter((row) =>
        preview.commandIds.includes(row.command.commandId),
      ),
      events,
      connection,
    );
    if (!expected.ok) return fail(expected.error.code, expected.error.retry);
    return isDeepStrictEqual(expected.value, preview)
      ? ok(undefined)
      : fail("content_conflict");
  }
  private connectionOptions(connection: Connection): ConnectionOptions {
    return {
      provider: connection.provider,
      config: {
        id: connection.connectionId,
        revision: String(connection.configRevision),
      },

      profile: connection.profile,
    };
  }
  selectConnection(
    caller: Caller,
    sessionId: string,
    connectionId: string,
    b: Budget,
    freshContext = false,
  ): Promise<Result<Session>> {
    const namespace = { ...caller, sessionId };
    return this.result(() =>
      this.admit(namespace, b, async () => {
        const session = requireValue(await this.store.session(namespace));
        const selected = await this.store.selectConnection(
          namespace,
          connectionId,
          session.revision,
          freshContext,
        );
        if (selected.ok) this.publish(namespace, { type: "resync_required" });
        return selected;
      }),
    );
  }
  private async ensureContext(
    session: Session,
    b: Budget,
  ): Promise<Result<Session>> {
    const selected = session.selectedConnectionId;
    if (!selected) return fail("connection_required");
    const connection = requireValue(
      await this.store.connection(session.namespace, selected),
    );
    if (connection.status !== "ready")
      return fail(
        connection.status === "authentication_required"
          ? "authentication_required"
          : "connection_required",
      );
    const stage = session.currentStageId ? activeStage(session) : undefined;
    const changed =
      session.freshContext ||
      !stage ||
      stage.connectionId !== selected ||
      stage.configRevision !== connection.configRevision;
    if (!changed) {
      if (session.status !== "active") return fail("context_unavailable");
      const runtime = this.runtimes.get(namespaceKey(session.namespace));
      if (runtime)
        return runtime.verified
          ? ok(session)
          : fail("reconciliation_required", "reconcile_first");
      return this.open(
        session.namespace,
        this.connectionOptions(connection),
        b,
        session,
        connection,
        true,
      );
    }
    const snapshot = await this.snapshot(session.namespace);
    if (snapshot.commands.some((row) => !isSettled(row)))
      return fail("connection_switch_pending");
    const key = namespaceKey(session.namespace),
      runtime = this.runtimes.get(key);
    if (runtime) {
      runtime.abort.abort();
      const stopped = await runtime.worker.close(b);
      if (!stopped.ok || !stopped.value.processStopped)
        return fail("reconciliation_required", "reconcile_first");
      if (!(await this.releaseRuntime(key, runtime)))
        return fail("reconciliation_required", "reconcile_first");
    }
    return this.open(
      session.namespace,
      this.connectionOptions(connection),
      b,
      session,
      connection,
      false,
    );
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
      this.admit(namespace, b, async () => {
        if (Object.keys(options).some((key) => key !== "connectionId"))
          return fail("invalid_input");
        const prefs = requireValue(await this.store.preferences(caller));
        const selected = options.connectionId ?? prefs.defaultConnectionId;
        if (selected) {
          const connection = requireValue(
            await this.store.connection(caller, selected),
          );
          if (connection.status !== "ready") return fail("connection_required");
        }
        const session = productSession(namespace, selected);
        requireValue(await this.store.create(session));
        return ok(session);
      }),
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
      this.admit(namespace, b, (b) =>
        (async () => {
          const previous = requireValue(await this.store.session(namespace));
          if (previous.status === "retired") return fail("session_gone");
          if (
            this.runtimes.get(namespaceKey(namespace))?.verified &&
            previous.status === "active" &&
            !this.blocked.has(namespaceKey(namespace))
          )
            return ok(previous);
          try {
            if (!previous.currentStageId) return ok(previous);
            const stage = activeStage(previous);
            const currentConnection = requireValue(
              await this.store.connection(namespace, stage.connectionId),
            );
            if (currentConnection.status !== "ready")
              return fail("connection_required");
            const connection = requireValue(
              await this.store.connection(
                namespace,
                stage.connectionId,
                stage.configRevision,
              ),
            );
            const result = await this.open(
              namespace,
              this.connectionOptions(connection),
              b,
              previous,
              connection,
              true,
            );
            if (!result.ok) await this.unavailable(namespace);
            return result;
          } catch (error) {
            await this.unavailable(namespace);
            throw error;
          }
        })(),
      ),
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
    if (
      ((this.closing || !this.callerAvailable(caller)) && !closing) ||
      b.signal.aborted
    )
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
      const result = await this.admit(
        namespace,
        b,
        async (b) => {
          if (b.signal.aborted)
            return fail<Receipt>("unavailable", "same_command");
          const operation = async (): Promise<Result<Receipt>> => {
            if (!closing && !this.callerAvailable(caller))
              return fail("unavailable");
            let session = requireValue(await this.store.session(namespace));
            const prior = await this.store.command(
              namespace,
              command.commandId,
            );
            if (!prior.ok) {
              if (command.input.type === "prompt" && command.input.history) {
                if (command.input.policy !== "queue_next")
                  return fail("invalid_input");
                const checked = await this.checkHistory(
                  session,
                  command.input.history,
                );
                if (!checked.ok) return checked;
              }
              if (
                command.input.type === "prompt" &&
                command.input.policy === "queue_next"
              ) {
                const prepared = await this.ensureContext(session, b);
                if (!prepared.ok) return prepared;
                session = prepared.value;
              }
              if (
                session.status !== "active" ||
                !this.runtimes.get(namespaceKey(namespace))?.verified
              )
                return fail<Receipt>(
                  "reconciliation_required",
                  "reconcile_first",
                );
              const snapshot = await this.snapshot(namespace),
                input = command.input;
              const ordinaryInput =
                input.type === "prompt" && input.policy === "queue_next";
              if (
                snapshot.commands.filter(
                  (row) =>
                    row.state === "accepted" &&
                    isOrdinary(row) === ordinaryInput,
                ).length >= (ordinaryInput ? this.queueLimit : 64)
              )
                return fail<Receipt>("limit_exceeded");
              if (
                input.type === "prompt" &&
                input.policy === "steer" &&
                (activeStage(session).capabilities.steer !== "supported" ||
                  input.targetRunId !==
                    activeStage(session).binding.nativeRunId)
              )
                return fail<Receipt>("unsupported_capability");
              if (
                input.type !== "prompt" &&
                input.generation !== activeStage(session).binding.generation
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
                    input.nativeRunId !==
                      activeStage(session).binding.nativeRunId)
                )
                  return fail<Receipt>("stale_binding");
                if (
                  target.dispatch &&
                  ["unsupported", "unknown"].includes(
                    activeStage(session).capabilities.cancellation,
                  )
                )
                  return fail<Receipt>("unsupported_capability");
              }
            }
            if (b.signal.aborted)
              return fail<Receipt>("unavailable", "same_command");
            const receipt = await this.store.accept({
              namespace,
              command,
              expectedRevision: session.revision,
              expectedGeneration: activeStage(session).binding.generation,
              nowMs: this.now(),
              retention: { retryWindowMs: 60000, receiptWindowMs: 86400000 },
              eventId: randomUUID(),
            });
            if (receipt.ok)
              await this.publishSince(namespace, session.lastSequence);
            return receipt;
          };
          return operation();
        },
        closing,
      );
      if (result.ok) this.kick(namespace);
      return result;
    });
  }
  private async snapshot(
    namespace: Namespace,
    deadline?: Deadline,
  ): Promise<{
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
      const read = () =>
        this.store.snapshotPage(namespace, {
          limit: 256,
          ...(page?.next ? { continuation: page.next } : {}),
        });
      page = requireValue(await (deadline ? deadline.wait(read) : read()));
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
      body.type === "command_accepted"
    )
      attemptId = undefined;
    return {
      schemaVersion: 5,
      kind: "event",
      namespace: session.namespace,
      eventId: randomUUID(),
      sequence: session.lastSequence + offset + 1,
      generation: activeStage(session).binding.generation,
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
      expectedGeneration: activeStage(session).binding.generation,
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
      if (
        session.status !== "active" ||
        runtime.abort.signal.aborted ||
        !runtime.verified
      )
        return;
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
          : !occupied && !this.closing && this.callerAvailable(namespace)
            ? ordinary
            : control;
      if (!record || runtime.inFlight.size >= 16) return;
      if (isOrdinary(record)) {
        if (this.closing || !this.callerAvailable(namespace)) return;
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
        originGeneration: activeStage(session).binding.generation,
        observerGeneration: activeStage(session).binding.generation,
        nativeSessionId: activeStage(session).binding.nativeSessionId,
        ...(activeStage(session).binding.nativeThreadId
          ? { nativeThreadId: activeStage(session).binding.nativeThreadId }
          : {}),
        certainty: "intent" as const,
        ...(!isOrdinary(record)
          ? {
              ...(activeStage(session).binding.nativeRunId
                ? { nativeRunId: activeStage(session).binding.nativeRunId }
                : {}),
              ...(record.command.input.type !== "prompt" &&
              activeStage(session).binding.nativeRequestId
                ? {
                    nativeRequestId:
                      activeStage(session).binding.nativeRequestId,
                  }
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
          generation: activeStage(session).binding.generation,
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
          schemaVersion: 5,
          kind: "interaction",
          namespace,
          commandId: record.command.commandId,
          generation: activeStage(session).binding.generation,
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
        if (next.state === "invalidated")
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
        : activeStage(session).binding;
    await this.commit(session, [next], events, {
      providerFacts: [proof],
      interactions,
      surfaces,
      session: replaceStage(
        {
          ...session,
          revision: session.revision + 1,
          lastSequence: session.lastSequence + events.length,
        },
        binding,
        undefined,
      ),
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
  private isolate(namespace: Namespace): Runtime | undefined {
    const key = namespaceKey(namespace),
      runtime = this.runtimes.get(key);
    if (runtime) {
      this.blocked.add(key);
      runtime.verified = undefined;
      runtime.abort.abort();
      runtime.worker.terminate();
    }
    return runtime;
  }
  private async unavailable(
    namespace: Namespace,
    b = budget(this.timeout),
  ): Promise<Result<void>> {
    const key = namespaceKey(namespace),
      runtime = this.isolate(namespace);
    const deadline = new Deadline(b);
    try {
      const session = await deadline.wait(() => this.store.session(namespace));
      if (!session.ok && session.error.code !== "session_gone")
        throw new HostFailure(session.error);
      if (
        session.ok &&
        session.value.status === "active" &&
        session.value.currentStageId
      ) {
        const s = session.value;
        requireValue(
          await deadline.wait(() =>
            this.store.recoverUnavailable({
              namespace,
              expectedRevision: s.revision,
              expectedGeneration: activeStage(s).binding.generation,
              eventId: randomUUID(),
            }),
          ),
        );
        await deadline.wait(() => this.publishSince(namespace, s.lastSequence));
      }
      if (runtime) {
        const stopped = requireValue(
          await deadline.wait(() => runtime.worker.close(deadline.budget())),
        );
        if (!stopped.processStopped) throw new Error("worker still present");
        if (!(await this.releaseRuntime(key, runtime)))
          throw new Error("worker cleanup incomplete");
        this.blocked.delete(key);
      }
      return ok(undefined);
    } catch (error) {
      this.diagnose("recovery", error);
      for (const stream of this.subscribers.get(key) ?? [])
        stream.end({ type: "resync_required" });
      return fail("unavailable", "same_command");
    } finally {
      // Even an unavailable Store cannot retain callbacks, tools, or a live owned worker.
      runtime?.worker.terminate();
      deadline.dispose();
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
    return this.closed || b.signal.aborted || !this.callerAvailable(caller)
      ? Promise.resolve(fail<SurfaceState>("unavailable"))
      : this.store.surface({ ...caller, sessionId }, instanceId);
  }
  snapshotPage(caller: Caller, sessionId: string, query: PageQuery, b: Budget) {
    return this.closed || b.signal.aborted || !this.callerAvailable(caller)
      ? Promise.resolve(fail<SnapshotPage>("unavailable"))
      : this.store.snapshotPage({ ...caller, sessionId }, query);
  }
  listSessions(caller: Caller, query: PageQuery, b: Budget) {
    return this.closed || b.signal.aborted || !this.callerAvailable(caller)
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
          !this.callerAvailable(caller) ||
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
  activateCaller(caller: Caller): void {
    this.suspended.delete(this.callerKey(caller));
  }
  async suspendCaller(caller: Caller, b: Budget): Promise<Result<void>> {
    const callerKey = this.callerKey(caller);
    this.suspended.add(callerKey);
    const result = await this.result(async () => {
      const deadline = new Deadline(b);
      try {
        const pending = [...this.admissions].filter(
          ([, owner]) => owner.caller === callerKey,
        );
        for (const [, owner] of pending) owner.abort.abort();
        await deadline.wait(() =>
          Promise.allSettled(pending.map(([task]) => task)),
        );
        for (const [key, runtime] of this.runtimes) {
          const [tenantId, principalId, authorityId] = JSON.parse(key);
          if (
            !runtime.verification ||
            this.callerKey({ tenantId, principalId, authorityId }) !== callerKey
          )
            continue;
          runtime.abort.abort();
          runtime.worker.terminate();
          const stopped = requireValue(
            await deadline.wait(() => runtime.worker.close(deadline.budget())),
          );
          if (!stopped.processStopped) return fail("unavailable");
          if (!(await this.releaseRuntime(key, runtime)))
            return fail("unavailable");
        }
        let continuation: string | undefined;
        do {
          const page = requireValue(
            await deadline.wait(() =>
              this.store.listSessions(caller, {
                limit: 256,
                ...(continuation ? { continuation } : {}),
              }),
            ),
          );
          for (const entry of page.items) {
            const namespace = entry.namespace;
            for (const stream of this.subscribers.get(
              namespaceKey(namespace),
            ) ?? [])
              stream.end({ type: "resync_required" });
            const snapshot = await deadline.wait(() =>
              this.snapshot(namespace),
            );
            const runtime = this.runtimes.get(namespaceKey(namespace));
            if (runtime?.verified && snapshot.session.status === "active") {
              for (const record of snapshot.commands.filter(
                (row) => isOrdinary(row) && !isSettled(row),
              )) {
                // Native cancellation is best effort; the durable fence below preserves uncertainty.
                await this.accept(
                  caller,
                  {
                    schemaVersion: 5,
                    kind: "command",
                    sessionId: namespace.sessionId,
                    commandId: randomUUID(),
                    expiresAtMs: this.now() + deadline.budget().timeoutMs,
                    input: {
                      type: "cancel",
                      targetCommandId: record.command.commandId,
                      generation: activeStage(snapshot.session).binding
                        .generation,
                      ...(record.dispatch?.nativeRunId
                        ? { nativeRunId: record.dispatch.nativeRunId }
                        : {}),
                    },
                  },
                  deadline.budget(),
                  true,
                );
              }
              const until =
                Date.now() + Math.min(1000, deadline.budget().timeoutMs);
              while (
                (runtime.inFlight.size ||
                  runtime.observing.size ||
                  this.tasks.size) &&
                Date.now() < until &&
                !b.signal.aborted
              )
                await new Promise((resolve) => setTimeout(resolve, 10));
            }
            await deadline.wait(() =>
              this.mailbox(namespace, async () => {
                requireValue(
                  await this.unavailable(namespace, deadline.budget()),
                );
                const session = requireValue(
                  await this.store.session(namespace),
                );
                if (!session.currentStageId || session.status === "retired")
                  return;
                requireValue(
                  await this.store.suspend({
                    namespace,
                    expectedRevision: session.revision,
                    expectedGeneration: activeStage(session).binding.generation,
                    eventId: randomUUID(),
                    nowMs: this.now(),
                    retention: {
                      retryWindowMs: 60000,
                      receiptWindowMs: 86400000,
                    },
                  }),
                );
              }),
            );
          }
          continuation = page.next;
        } while (continuation);
        return ok(undefined);
      } finally {
        deadline.dispose();
      }
    });
    if (!result.ok) this.suspended.delete(callerKey);
    return result;
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
    if (
      !Number.isSafeInteger(b.timeoutMs) ||
      b.timeoutMs < 1 ||
      b.timeoutMs > 2147483647
    )
      return fail("invalid_input");
    this.closing = true;
    this.deliveryAbort.abort();
    const deadline = new Deadline(b);
    for (const { abort } of this.admissions.values()) abort.abort();
    for (const timer of this.retryTimers) clearTimeout(timer);
    this.retryTimers.clear();
    try {
      await deadline.wait(() =>
        Promise.allSettled([...this.admissions.keys()]),
      );
      for (const [key, runtime] of this.runtimes) {
        if (runtime.verification || runtime.abort.signal.aborted) continue;
        const [tenantId, principalId, authorityId, sessionId] = JSON.parse(key);
        const namespace = { tenantId, principalId, authorityId, sessionId };
        const snapshot = await this.snapshot(namespace, deadline);
        for (const record of snapshot.commands.filter(
          (row) => isOrdinary(row) && row.dispatch && !isSettled(row),
        )) {
          requireValue(
            await deadline.wait(() =>
              this.accept(
                namespace,
                {
                  schemaVersion: 5,
                  kind: "command",
                  sessionId,
                  commandId: randomUUID(),
                  expiresAtMs: this.now() + deadline.budget().timeoutMs,
                  input: {
                    type: "cancel",
                    targetCommandId: record.command.commandId,
                    generation: activeStage(snapshot.session).binding
                      .generation,
                    ...(record.dispatch?.nativeRunId
                      ? { nativeRunId: record.dispatch.nativeRunId }
                      : {}),
                  },
                },
                deadline.budget(),
                true,
              ),
            ),
          );
        }
        runtime.worker.onFailure = undefined;
      }
      // Leave time for cancellation observations, bounded by the same shutdown deadline.
      const settleUntil =
        Date.now() +
        Math.min(1000, Math.max(0, deadline.budget().timeoutMs - 1000));
      while (this.tasks.size && Date.now() < settleUntil)
        await deadline.wait(
          () => new Promise((resolve) => setTimeout(resolve, 10)),
        );
      for (const [key, runtime] of [...this.runtimes]) {
        if (runtime.verification) {
          const result = requireValue(
            await deadline.wait(() => runtime.worker.close(deadline.budget())),
          );
          if (!result.processStopped) throw new Error("worker still present");
          if (!(await this.releaseRuntime(key, runtime)))
            throw new Error("worker cleanup incomplete");
          continue;
        }
        runtime.abort.abort();
        runtime.worker.terminate();
        const [tenantId, principalId, authorityId, sessionId] = JSON.parse(key);
        const namespace = { tenantId, principalId, authorityId, sessionId };
        requireValue(
          await deadline.wait(() =>
            this.mailbox(namespace, () =>
              this.unavailable(namespace, deadline.budget()),
            ),
          ),
        );
      }
      for (const dispose of [...this.disposals])
        if (!(await deadline.wait(() => this.releaseDispose(dispose))))
          throw new Error("worker cleanup incomplete");
      const result = await deadline.wait(() =>
        this.store.close(deadline.budget()),
      );
      if (result.ok) this.closed = true;
      return result;
    } catch (error) {
      this.diagnose("close", error);
      return fail("unavailable", "same_command");
    } finally {
      // This synchronous fallback runs even when Store/admission/mailbox awaits never settle.
      for (const [key, runtime] of this.runtimes) {
        this.blocked.add(key);
        runtime.verified = undefined;
        runtime.abort.abort();
        runtime.worker.terminate();
      }
      for (const listeners of this.subscribers.values())
        for (const stream of listeners) stream.end();
      this.subscribers.clear();
      deadline.dispose();
    }
  }
}
export async function createHost(
  options: HostOptions,
): Promise<Result<SessionHost>> {
  return SessionHost.create(options);
}
