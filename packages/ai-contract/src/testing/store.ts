import canonicalize from "canonicalize";
import {
  decode,
  boundedJson,
  fingerprint,
  deliveryFingerprint,
  ContractError,
} from "../codec.js";
import type {
  AcceptCommand,
  Page,
  Result,
  SessionCommit,
  SessionStore,
  Snapshot,
} from "../ports.js";
import type {
  CommandRecord,
  Counter,
  Delivery,
  Event,
  Id,
  Namespace,
  Session,
} from "../wire.js";
export const fixtureLimits = {
  maxBytes: 262144,
  maxTextBytes: 131072,
  maxDepth: 32,
  maxNodes: 16384,
};
export const ok = <T>(value: T): Result<T> => ({ ok: true, value });
export const fail = <T = never>(
  code: import("../wire.js").ErrorCode,
  retry: import("../wire.js").Retry = "never",
): Result<T> => ({ ok: false, error: { code, retry } });
const clone = <T>(value: T): T => structuredClone(value);
export const namespaceKey = (n: Namespace): string =>
  JSON.stringify([n.tenantId, n.principalId, n.authorityId, n.sessionId]);
const valid = (v: unknown) =>
  decode(boundedJson(v, fixtureLimits), fixtureLimits);
const same = (a: unknown, b: unknown) => canonicalize(a) === canonicalize(b);
const sessionIdentity = ({
  nativeRunId: _run,
  nativeRequestId: _request,
  ...identity
}: Session["binding"]) => identity;
interface State {
  session: Session;
  commands: Map<Id, CommandRecord>;
  events: Event[];
  interactions: Map<Id, Snapshot["interactions"][number]>;
  deliveries: Map<Id, Delivery>;
  surfaces: Map<Id, Snapshot["surfaces"][number]>;
}
/** Deterministic contract test double. No disk, crash durability, locks, workers or leases. */
export class MemorySessionStore implements SessionStore {
  readonly evidence = "memory_test_double" as const;
  private states = new Map<string, State>();
  private retiredIds = new Set<string>();
  /** Inject a transaction failure before publication, without partially mutating state. */
  failNextCommit = false;
  async create(session: Session): Promise<Result<void>> {
    try {
      valid(session);
    } catch {
      return fail("invalid_input");
    }
    const key = namespaceKey(session.namespace);
    if (this.states.has(key) || this.retiredIds.has(key))
      return fail("content_conflict");
    if (
      session.revision !== 0 ||
      session.lastSequence !== 0 ||
      session.status !== "active"
    )
      return fail("invalid_input");
    this.states.set(key, {
      session: clone(session),
      commands: new Map(),
      events: [],
      interactions: new Map(),
      deliveries: new Map(),
      surfaces: new Map(),
    });
    return ok(undefined);
  }
  async session(namespace: Namespace): Promise<Result<Session>> {
    const state = this.states.get(namespaceKey(namespace));
    return state ? ok(clone(state.session)) : fail("session_gone");
  }
  async command(namespace: Namespace, id: Id): Promise<Result<CommandRecord>> {
    const state = this.states.get(namespaceKey(namespace));
    if (!state) return fail("session_gone");
    const c = state.commands.get(id);
    return c ? ok(clone(c)) : fail("unavailable");
  }
  async accept(
    input: AcceptCommand,
  ): Promise<Result<import("../wire.js").Receipt>> {
    const state = this.states.get(namespaceKey(input.namespace));
    if (!state) return fail("session_gone");
    try {
      valid(input.command);
      valid(input.event);
    } catch {
      return fail("invalid_input");
    }
    if (input.command.sessionId !== input.namespace.sessionId)
      return fail("permission_denied");
    const digest = fingerprint(input.command, fixtureLimits),
      prior = state.commands.get(input.command.commandId);
    if (prior) {
      if (prior.receipt.contentHash !== digest) return fail("content_conflict");
      return input.nowMs <= prior.receipt.receiptUntilMs
        ? ok(clone(prior.receipt))
        : fail("expired");
    }
    if (state.session.status !== "active") return fail("session_gone");
    if (
      !Number.isSafeInteger(input.nowMs) ||
      input.nowMs < 0 ||
      input.command.expiresAtMs < input.nowMs
    )
      return fail("expired");
    const { retryWindowMs, receiptWindowMs } = input.retention;
    if (
      !Number.isSafeInteger(retryWindowMs) ||
      retryWindowMs < 1 ||
      !Number.isSafeInteger(receiptWindowMs) ||
      receiptWindowMs < retryWindowMs ||
      !Number.isSafeInteger(input.nowMs + receiptWindowMs)
    )
      return fail("invalid_input");
    const check = this.check(
      state,
      input.expectedRevision,
      input.expectedGeneration,
    );
    if (!check.ok) return check;
    const receipt: import("../wire.js").Receipt = {
      schemaVersion: 2,
      kind: "receipt",
      namespace: clone(input.namespace),
      commandId: input.command.commandId,
      contentHash: digest,
      acceptedAtMs: input.nowMs,
      retryUntilMs: Math.min(
        input.command.expiresAtMs,
        input.nowMs + retryWindowMs,
      ),
      receiptUntilMs: input.nowMs + receiptWindowMs,
      acceptedRevision: state.session.revision + 1,
    };
    const record: CommandRecord = {
      schemaVersion: 2,
      kind: "commandRecord",
      command: clone(input.command),
      receipt,
      state: "accepted",
    };
    const interactions: Snapshot["interactions"][number][] = [];
    if (input.command.input.type === "respond") {
      const request = input.command.input;
      const interaction = state.interactions.get(request.interactionId);
      if (!interaction || interaction.status === "unavailable")
        return fail("unavailable");
      if (
        interaction.generation !== request.generation ||
        request.generation !== input.expectedGeneration ||
        interaction.nativeRunId !== request.nativeRunId
      )
        return fail("stale_binding");
      if (interaction.status === "answered") return fail("already_answered");
      if (
        interaction.status === "expired" ||
        interaction.expiresAtMs < input.nowMs
      )
        return fail("expired");
      interactions.push({
        ...interaction,
        status: "answered",
        responseCommandId: input.command.commandId,
      });
    }
    const result = await this.commit({
      namespace: input.namespace,
      expectedRevision: input.expectedRevision,
      expectedGeneration: input.expectedGeneration,
      session: {
        ...state.session,
        revision: state.session.revision + 1,
        lastSequence: state.session.lastSequence + 1,
      },
      commands: [record],
      events: [input.event],
      interactions,
      deliveries: [],
      surfaces: [],
    });
    return result.ok ? ok(clone(receipt)) : result;
  }
  private check(state: State, revision: Counter, generation: Id): Result<void> {
    if (state.session.status !== "active") return fail("session_gone");
    if (state.session.binding.generation !== generation)
      return fail("stale_binding");
    if (state.session.revision !== revision)
      return fail("revision_conflict", "same_command");
    return ok(undefined);
  }
  async commit(batch: SessionCommit): Promise<Result<void>> {
    const state = this.states.get(namespaceKey(batch.namespace));
    if (!state) return fail("session_gone");
    const check = this.check(
      state,
      batch.expectedRevision,
      batch.expectedGeneration,
    );
    if (!check.ok) return check;
    try {
      for (const row of [
        batch.session,
        ...batch.commands,
        ...batch.events,
        ...batch.interactions,
        ...batch.deliveries,
        ...batch.surfaces,
      ])
        valid(row);
    } catch (error) {
      if (error instanceof ContractError) return fail("invalid_input");
      throw error;
    }
    if (
      namespaceKey(batch.session.namespace) !== namespaceKey(batch.namespace) ||
      batch.session.revision !== state.session.revision + 1 ||
      batch.session.lastSequence !==
        state.session.lastSequence + batch.events.length ||
      batch.session.status !== "active"
    )
      return fail("invalid_input");
    if (
      !same(
        sessionIdentity(batch.session.binding),
        sessionIdentity(state.session.binding),
      ) ||
      !same(batch.session.capabilities, state.session.capabilities)
    )
      return fail("stale_binding");
    const copy = clone(state);
    for (const c of batch.commands) {
      if (
        namespaceKey(c.receipt.namespace) !== namespaceKey(batch.namespace) ||
        c.command.sessionId !== batch.namespace.sessionId
      )
        return fail("permission_denied");
      const old = copy.commands.get(c.command.commandId);
      if (old) {
        if (
          !same(old.receipt, c.receipt) ||
          fingerprint(old.command, fixtureLimits) !==
            fingerprint(c.command, fixtureLimits)
        )
          return fail("content_conflict");
        const allowed: Record<string, readonly string[]> = {
          accepted: ["accepted", "dispatching", "terminal"],
          dispatching: [
            "dispatching",
            "running",
            "terminal",
            "reconciliation_required",
          ],
          running: ["running", "terminal", "reconciliation_required"],
          reconciliation_required: [
            "reconciliation_required",
            "running",
            "terminal",
          ],
          terminal: ["terminal"],
        };
        if (
          !allowed[old.state].includes(c.state) ||
          (old.state === "terminal" && !same(old, c))
        )
          return fail("content_conflict");
      } else if (
        c.state !== "accepted" ||
        c.receipt.acceptedRevision !== batch.session.revision ||
        copy.commands.has(c.command.commandId)
      )
        return fail("invalid_input");
      if (
        c.dispatch &&
        (c.dispatch.generation !== batch.expectedGeneration ||
          c.dispatch.nativeSessionId !== state.session.binding.nativeSessionId)
      )
        return fail("stale_binding");
      if (
        old?.dispatch &&
        c.dispatch &&
        ((old.dispatch.nativeRunId !== undefined &&
          old.dispatch.nativeRunId !== c.dispatch.nativeRunId) ||
          (old.dispatch.nativeRequestId !== undefined &&
            old.dispatch.nativeRequestId !== c.dispatch.nativeRequestId))
      )
        return fail("stale_binding");
      copy.commands.set(c.command.commandId, clone(c));
    }
    const oldBinding = state.session.binding,
      nextBinding = batch.session.binding;
    const coordinatesMatch = (
      dispatch: CommandRecord["dispatch"],
      binding: Session["binding"],
    ) =>
      dispatch !== undefined &&
      dispatch.nativeRunId === binding.nativeRunId &&
      dispatch.nativeRequestId === binding.nativeRequestId;
    if (
      oldBinding.nativeRunId !== nextBinding.nativeRunId ||
      oldBinding.nativeRequestId !== nextBinding.nativeRequestId
    ) {
      const records = [...copy.commands.values()];
      if (
        nextBinding.nativeRunId !== undefined ||
        nextBinding.nativeRequestId !== undefined
      ) {
        if (
          !records.some(
            (c) =>
              (c.state === "dispatching" || c.state === "running") &&
              c.dispatch?.certainty === "submitted" &&
              coordinatesMatch(c.dispatch, nextBinding),
          )
        )
          return fail("stale_binding");
      } else {
        const original = records.filter((c) =>
          coordinatesMatch(c.dispatch, oldBinding),
        );
        if (!original.length || original.some((c) => c.state !== "terminal"))
          return fail("stale_binding");
      }
    }
    const ids = new Set(copy.events.map((e) => e.eventId));
    for (const [i, event] of batch.events.entries()) {
      if (
        namespaceKey(event.namespace) !== namespaceKey(batch.namespace) ||
        event.generation !== batch.expectedGeneration ||
        event.sequence !== state.session.lastSequence + i + 1 ||
        ids.has(event.eventId) ||
        !copy.commands.has(event.commandId)
      )
        return fail("invalid_input");
      ids.add(event.eventId);
      copy.events.push(clone(event));
    }
    for (const row of batch.interactions) {
      const source = copy.commands.get(row.commandId);
      if (
        namespaceKey(row.namespace) !== namespaceKey(batch.namespace) ||
        row.generation !== batch.expectedGeneration ||
        !source?.dispatch ||
        source.dispatch.generation !== row.generation ||
        source.dispatch.nativeRunId !== row.nativeRunId ||
        source.dispatch.nativeRequestId !== row.nativeRequestId
      )
        return fail("stale_binding");
      const old = copy.interactions.get(row.interactionId);
      if (old) {
        const { status: _a, responseCommandId: _b, ...identity } = old;
        const { status: _c, responseCommandId: _d, ...nextIdentity } = row;
        if (!same(identity, nextIdentity)) return fail("stale_binding");
        if (old.status !== "pending" && !same(old, row))
          return fail("already_answered");
      } else if (row.status === "answered") return fail("invalid_input");
      if (row.status === "answered") {
        const response = copy.commands.get(row.responseCommandId!);
        if (
          !response ||
          response.command.input.type !== "respond" ||
          response.command.input.interactionId !== row.interactionId ||
          response.command.input.generation !== row.generation ||
          response.command.input.nativeRunId !== row.nativeRunId ||
          response.receipt.acceptedAtMs > row.expiresAtMs
        )
          return fail("invalid_input");
      }
      copy.interactions.set(row.interactionId, clone(row));
    }
    for (const row of batch.deliveries) {
      if (
        namespaceKey(row.namespace) !== namespaceKey(batch.namespace) ||
        !ids.has(row.eventId) ||
        row.contentHash !==
          deliveryFingerprint(
            copy.events.find((e) => e.eventId === row.eventId)!,
            row.target,
            fixtureLimits,
          )
      )
        return fail("invalid_input");
      const old = copy.deliveries.get(row.operationId);
      if (
        old &&
        (old.contentHash !== row.contentHash ||
          old.target !== row.target ||
          old.eventId !== row.eventId ||
          old.retry !== row.retry ||
          row.attempts < old.attempts ||
          (old.status === "delivered" && row.status !== "delivered"))
      )
        return fail("content_conflict");
      copy.deliveries.set(row.operationId, clone(row));
    }
    for (const row of batch.surfaces) {
      if (
        namespaceKey(row.namespace) !== namespaceKey(batch.namespace) ||
        row.generation !== batch.expectedGeneration ||
        !copy.interactions.has(row.interactionId)
      )
        return fail("invalid_input");
      copy.surfaces.set(row.surfaceInstanceId, clone(row));
    }
    if (this.failNextCommit) {
      this.failNextCommit = false;
      return fail("unavailable", "same_command");
    }
    copy.session = clone(batch.session);
    this.states.set(namespaceKey(batch.namespace), copy);
    return ok(undefined);
  }
  async snapshot(
    namespace: Namespace,
    limit: number = 1024,
  ): Promise<Result<Snapshot>> {
    const s = this.states.get(namespaceKey(namespace));
    if (!s) return fail("session_gone");
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > 1024)
      return fail("invalid_input");
    if (
      s.events.length +
        s.commands.size +
        s.interactions.size +
        s.surfaces.size >
      limit
    )
      return fail("limit_exceeded");
    return ok(
      clone({
        session: s.session,
        cursor: s.session.lastSequence,
        events: s.events,
        commands: [...s.commands.values()],
        interactions: [...s.interactions.values()],
        surfaces: [...s.surfaces.values()],
      }),
    );
  }
  async events(
    namespace: Namespace,
    after: Counter,
    limit: number,
  ): Promise<Result<readonly Event[]>> {
    const s = this.states.get(namespaceKey(namespace));
    if (!s) return fail("session_gone");
    if (
      !Number.isSafeInteger(after) ||
      after < 0 ||
      after > s.session.lastSequence
    )
      return fail("cursor_expired");
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > 1024)
      return fail("invalid_input");
    return ok(
      clone(s.events.filter((e) => e.sequence > after).slice(0, limit)),
    );
  }
  async recovery(limit: number, after?: Id): Promise<Page<CommandRecord>> {
    return this.page(
      [...this.states.values()]
        .flatMap((s) => [...s.commands.values()])
        .filter((c) => c.state !== "terminal"),
      (c) => namespaceKey(c.receipt.namespace) + "/" + c.command.commandId,
      limit,
      after,
    );
  }
  async deliveries(
    limit: number,
    nowMs: Counter,
    after?: Id,
  ): Promise<Page<Delivery>> {
    return this.page(
      [...this.states.values()]
        .flatMap((s) => [...s.deliveries.values()])
        .filter((d) => d.status === "pending" && d.nextAttemptAtMs <= nowMs),
      (d) => namespaceKey(d.namespace) + "/" + d.operationId,
      limit,
      after,
    );
  }
  private page<T>(
    rows: T[],
    key: (row: T) => string,
    limit: number,
    after?: string,
  ): Page<T> {
    if (!Number.isInteger(limit) || limit < 1 || limit > 1024)
      throw new ContractError("configuration");
    const sorted = rows
      .sort((a, b) => (key(a) < key(b) ? -1 : 1))
      .filter((r) => after === undefined || key(r) > after);
    const items = sorted.slice(0, limit);
    return clone({
      items,
      ...(sorted.length > limit ? { next: key(items.at(-1)!) } : {}),
    });
  }
  async retire(
    namespace: Namespace,
    revision: Counter,
    generation: Id,
  ): Promise<Result<void>> {
    const s = this.states.get(namespaceKey(namespace));
    if (!s) return fail("session_gone");
    const check = this.check(s, revision, generation);
    if (!check.ok) return check;
    if (
      [...s.commands.values()].some((c) => c.state !== "terminal") ||
      [...s.interactions.values()].some((i) => i.status === "pending")
    )
      return fail("reconciliation_required", "reconcile_first");
    s.session = { ...s.session, status: "retired", revision: revision + 1 };
    return ok(undefined);
  }
  async pruneRetired(nowMs: Counter): Promise<number> {
    let n = 0;
    for (const [key, s] of this.states) {
      if (
        s.session.status === "retired" &&
        [...s.commands.values()].every(
          (c) => c.receipt.receiptUntilMs < nowMs,
        ) &&
        [...s.deliveries.values()].every((d) => d.status === "delivered")
      ) {
        this.states.delete(key);
        this.retiredIds.add(key);
        n++;
      }
    }
    return n;
  }
}
