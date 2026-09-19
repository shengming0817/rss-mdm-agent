import { ReadViews, readSnapshotPage, readSessionPage } from "../read-views.js";
import type {
  AcceptCommand,
  Budget,
  Page,
  Result,
  SessionCommit,
  SessionRebind,
  RecoveryUnavailable,
  SessionStore,
  Caller,
  Clock,
} from "../ports.js";
import type {
  CommandRecord,
  Event,
  Counter,
  Delivery,
  Id,
  Namespace,
  Session,
  SurfaceState,
  SnapshotPage,
  SessionPage,
  PageQuery,
} from "../wire.js";
import {
  acceptCommand,
  commitSession,
  createState,
  rebindSession,
  recoverUnavailable,
  retireSession,
  canPrune,
  defaultLimits,
  namespaceKey,
  ok,
  fail,
  isSettled,
  type SessionState,
} from "../transitions.js";
export const fixtureLimits = defaultLimits;
export { ok, fail, namespaceKey } from "../transitions.js";
const clone = <T>(v: T): T => structuredClone(v);
/** Deterministic storage double; production transition rules, no durability claim. */
export class MemorySessionStore implements SessionStore {
  readonly evidence = "memory_test_double" as const;
  private closed = false;
  private readonly views: ReadViews;
  constructor(options: { clock?: Clock; snapshotTtlMs?: number } = {}) {
    const ttl = options.snapshotTtlMs ?? 30000;
    if (!Number.isSafeInteger(ttl) || ttl <= 0)
      throw new TypeError("snapshot ttl");
    this.views = new ReadViews(options.clock ?? { now: () => Date.now() }, ttl);
  }
  private states = new Map<string, SessionState>();
  private retiredIds = new Set<string>();
  failNextCommit = false;
  failNextQuery = false;
  private query<T>(action: () => Result<T>): Result<T> {
    try {
      return action();
    } catch {
      return fail("invalid_input");
    }
  }
  private apply(
    namespace: Namespace,
    transition: (s: SessionState) => Result<SessionState>,
  ): Result<void> {
    if (this.closed) return fail("unavailable");
    try {
      const key = namespaceKey(namespace),
        before = this.states.get(key);
      if (!before) return fail("session_gone");
      const result = transition(before);
      if (!result.ok) return result;
      if (this.failNextCommit) {
        this.failNextCommit = false;
        return fail("unavailable", "same_command");
      }
      this.states.set(key, result.value);
      return ok(undefined);
    } catch {
      return fail("invalid_input");
    }
  }
  async create(session: Session): Promise<Result<void>> {
    if (this.closed) return fail("unavailable");
    try {
      const key = namespaceKey(session.namespace);
      if (this.states.has(key) || this.retiredIds.has(key))
        return fail("content_conflict");
      const result = createState(session);
      if (!result.ok) return result;
      this.states.set(key, result.value);
      return ok(undefined);
    } catch {
      return fail("invalid_input");
    }
  }
  async session(namespace: Namespace): Promise<Result<Session>> {
    return this.query(() => {
      if (this.closed) return fail("unavailable");
      const state = this.states.get(namespaceKey(namespace));
      return state ? ok(clone(state.session)) : fail("session_gone");
    });
  }
  async command(namespace: Namespace, id: Id): Promise<Result<CommandRecord>> {
    return this.query(() => {
      if (this.closed) return fail("unavailable");
      const state = this.states.get(namespaceKey(namespace));
      if (!state) return fail("session_gone");
      const row = state.commands.get(id);
      return row ? ok(clone(row)) : fail("unavailable");
    });
  }
  async surface(namespace: Namespace, id: Id): Promise<Result<SurfaceState>> {
    return this.query(() => {
      if (this.closed) return fail("unavailable");
      const state = this.states.get(namespaceKey(namespace));
      if (!state || state.session.status !== "active")
        return fail("session_gone");
      const row = state.surfaces.get(id);
      return row ? ok(clone(row)) : fail("stale_binding");
    });
  }
  async accept(
    input: AcceptCommand,
  ): Promise<Result<import("../wire.js").Receipt>> {
    let receipt: import("../wire.js").Receipt | undefined;
    const result = this.apply(input.namespace, (state) => {
      const accepted = acceptCommand(state, input);
      if (!accepted.ok) return accepted;
      receipt = accepted.value.receipt;
      return ok(accepted.value.state);
    });
    return result.ok ? ok(receipt!) : result;
  }
  async commit(batch: SessionCommit): Promise<Result<void>> {
    return this.apply(batch.namespace, (s) => commitSession(s, batch));
  }
  async rebind(input: SessionRebind): Promise<Result<Session>> {
    const result = this.apply(input.namespace, (s) => rebindSession(s, input));
    return result.ok ? this.session(input.namespace) : result;
  }
  async recoverUnavailable(
    input: RecoveryUnavailable,
  ): Promise<Result<Session>> {
    const result = this.apply(input.namespace, (s) =>
      recoverUnavailable(s, input),
    );
    return result.ok ? this.session(input.namespace) : result;
  }
  async snapshotPage(
    namespace: Namespace,
    query: PageQuery,
  ): Promise<Result<SnapshotPage>> {
    return this.query(() => {
      if (this.closed) return fail("unavailable");
      const state = this.states.get(namespaceKey(namespace));
      if (!state) return fail("session_gone");
      if (this.failNextQuery) {
        this.failNextQuery = false;
        return fail("unavailable", "same_command");
      }
      return readSnapshotPage(
        this.views,
        `snapshot:${namespaceKey(namespace)}`,
        query,
        () => ({
          session: state.session,
          records: [
            ...state.events,
            ...state.commands.values(),
            ...state.interactions.values(),
            ...state.surfaces.values(),
          ],
        }),
        fixtureLimits,
      );
    });
  }
  async listSessions(
    caller: Caller,
    query: PageQuery,
  ): Promise<Result<SessionPage>> {
    return this.query(() => {
      if (this.closed) return fail("unavailable");
      const scope = JSON.stringify([
        caller.tenantId,
        caller.principalId,
        caller.authorityId,
      ]);
      namespaceKey({ ...caller, sessionId: "scope-validation" });
      return readSessionPage(
        this.views,
        `list:${scope}`,
        query,
        () =>
          [...this.states.values()]
            .map((s) => s.session)
            .filter(
              (s) =>
                s.status !== "retired" &&
                s.namespace.tenantId === caller.tenantId &&
                s.namespace.principalId === caller.principalId &&
                s.namespace.authorityId === caller.authorityId,
            )
            .sort((a, b) =>
              a.namespace.sessionId < b.namespace.sessionId
                ? -1
                : a.namespace.sessionId > b.namespace.sessionId
                  ? 1
                  : 0,
            ),
        fixtureLimits,
      );
    });
  }
  async events(
    namespace: Namespace,
    after: Counter,
    limit: number,
  ): Promise<Result<readonly Event[]>> {
    return this.query(() => {
      if (this.closed) return fail("unavailable");
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
    });
  }
  async recovery(
    limit: number,
    after?: Id,
  ): Promise<Result<Page<CommandRecord>>> {
    if (this.closed) return fail("unavailable");
    if (this.failNextQuery) {
      this.failNextQuery = false;
      return fail("unavailable", "same_command");
    }
    return this.page(
      [...this.states.values()]
        .flatMap((s) => [...s.commands.values()])
        .filter((c) => !isSettled(c)),
      (c) => namespaceKey(c.receipt.namespace) + "/" + c.command.commandId,
      limit,
      after,
    );
  }
  async deliveries(
    limit: number,
    nowMs: Counter,
    after?: Id,
  ): Promise<Result<Page<Delivery>>> {
    if (this.closed) return fail("unavailable");
    if (this.failNextQuery) {
      this.failNextQuery = false;
      return fail("unavailable", "same_command");
    }
    if (!Number.isSafeInteger(nowMs) || nowMs < 0) return fail("invalid_input");
    return this.page(
      [...this.states.values()]
        .flatMap((s) => [...s.deliveries.values()])
        .filter((d) => d.status !== "delivered" && d.nextAttemptAtMs <= nowMs),
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
  ): Result<Page<T>> {
    if (!Number.isInteger(limit) || limit < 1 || limit > 1024)
      return fail("invalid_input");
    if (
      after !== undefined &&
      (typeof after !== "string" || after.length < 1 || after.length > 2048)
    )
      return fail("invalid_input");
    const sorted = rows
      .sort((a, b) => (key(a) < key(b) ? -1 : 1))
      .filter((r) => after === undefined || key(r) > after);
    const items = sorted.slice(0, limit);
    return ok(
      clone({
        items,
        ...(sorted.length > limit ? { next: key(items.at(-1)!) } : {}),
      }),
    );
  }

  async retire(
    namespace: Namespace,
    revision: Counter,
    generation: Id,
  ): Promise<Result<void>> {
    return this.apply(namespace, (s) => retireSession(s, revision, generation));
  }
  async pruneRetired(nowMs: Counter): Promise<Result<number>> {
    if (this.closed) return fail("unavailable");
    if (!Number.isSafeInteger(nowMs) || nowMs < 0) return fail("invalid_input");
    let n = 0;
    for (const [key, s] of this.states)
      if (canPrune(s, nowMs)) {
        this.states.delete(key);
        this.retiredIds.add(key);
        n++;
      }
    return ok(n);
  }
  async close(_budget: Budget): Promise<Result<void>> {
    this.closed = true;
    this.views.clear();
    return ok(undefined);
  }
}
