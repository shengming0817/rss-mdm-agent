import { boundedJson, ContractError, type Limits } from "./codec.js";
import type {
  SnapshotPage,
  SessionPage,
  Event,
  CommandRecord,
  SurfaceState,
  Session,
} from "./wire.js";
const ok = <T>(value: T): Result<T> => ({ ok: true, value });
const fail = (code: "cursor_expired" | "limit_exceeded"): Result<never> => ({
  ok: false,
  error: { code, retry: "never" },
});
export type SnapshotRecords = {
  session: Session;
  records: (
    | Event
    | CommandRecord
    | SnapshotPage["interactions"][number]
    | SurfaceState
  )[];
};
import type { Clock, Result } from "./ports.js";
import type { PageQuery } from "./wire.js";

/** Process-local immutable pages. Restart/expiry invalidates continuation; durable events remain replayable. */
export class ReadViews {
  private views = new Map<
    string,
    {
      scope: string;
      expires: number;
      limit: number;
      value: unknown;
      bytes: number;
      continued: boolean;
    }
  >();
  private retainedBytes = 0;
  private tokens = new Map<
    string,
    { id: string; offset: number; index: number }
  >();
  constructor(
    private clock: Clock,
    private ttlMs: number,
  ) {}
  read<T>(
    scope: string,
    query: PageQuery,
    capture: () => T,
  ): Result<{ id: string; offset: number; index: number; value: T }> {
    if (
      !Number.isSafeInteger(query.limit) ||
      query.limit < 1 ||
      query.limit > 256
    )
      return { ok: false, error: { code: "invalid_input", retry: "never" } };
    const now = this.clock.now();
    for (const [id, view] of this.views)
      if (view.expires < now) {
        this.retainedBytes -= view.bytes;
        this.views.delete(id);
      }
    for (const [token, row] of this.tokens)
      if (!this.views.has(row.id)) this.tokens.delete(token);
    if (query.continuation) {
      const token = this.tokens.get(query.continuation);
      const view = token && this.views.get(token.id);
      if (!token || !view || view.scope !== scope || view.limit !== query.limit)
        return { ok: false, error: { code: "cursor_expired", retry: "never" } };
      return {
        ok: true,
        value: { ...token, value: structuredClone(view.value) as T },
      };
    }
    if (this.views.size >= 128)
      return { ok: false, error: { code: "limit_exceeded", retry: "never" } };
    const id = crypto.randomUUID(),
      value = structuredClone(capture());
    let bytes: number;
    try {
      bytes = new TextEncoder().encode(
        boundedJson(value, {
          maxBytes: 16 * 1024 * 1024,
          maxTextBytes: 16 * 1024 * 1024,
          maxNodes: 1048576,
          maxDepth: 64,
        }),
      ).length;
    } catch (error) {
      if (error instanceof ContractError && error.code === "limit")
        return fail("limit_exceeded");
      throw error;
    }
    if (this.retainedBytes + bytes > 16 * 1024 * 1024)
      return { ok: false, error: { code: "limit_exceeded", retry: "never" } };
    this.retainedBytes += bytes;
    this.views.set(id, {
      bytes,
      scope,
      continued: false,
      expires: now + this.ttlMs,
      limit: query.limit,
      value,
    });
    return {
      ok: true,
      value: { id, offset: 0, index: 0, value: structuredClone(value) },
    };
  }
  next(id: string, offset: number, index: number): string {
    for (const [token, row] of this.tokens)
      if (row.id === id && row.offset === offset && row.index === index)
        return token;
    const token = crypto.randomUUID();
    this.tokens.set(token, { id, offset, index });
    return token;
  }
  /** Preserve issued continuation tokens for retries, release unused captures. */
  finish(id: string, hasContinuation: boolean): void {
    const view = this.views.get(id);
    if (!view) return;
    view.continued ||= hasContinuation;
    if (!view.continued) {
      this.retainedBytes -= view.bytes;
      this.views.delete(id);
      for (const [token, row] of this.tokens)
        if (row.id === id) this.tokens.delete(token);
    }
  }
  clear(): void {
    this.retainedBytes = 0;
    this.views.clear();
    this.tokens.clear();
  }
}

export function readSnapshotPage(
  views: ReadViews,
  scope: string,
  query: PageQuery,
  capture: () => SnapshotRecords,
  limits: Limits,
): Result<SnapshotPage> {
  const read = views.read(scope, query, capture);
  if (!read.ok) return read;
  const { id, offset, index, value } = read.value;
  if (offset > value.records.length) return fail("cursor_expired");
  let count = Math.min(query.limit, value.records.length - offset);
  for (;;) {
    const records = value.records.slice(offset, offset + count);
    const page: SnapshotPage = {
      schemaVersion: 2,
      kind: "snapshotPage",
      snapshotId: id,
      pageIndex: index,
      session: value.session,
      cursor: value.session.lastSequence,
      events: records.filter((r): r is Event => r.kind === "event"),
      commands: records.filter(
        (r): r is CommandRecord => r.kind === "commandRecord",
      ),
      interactions: records.filter(
        (r): r is SnapshotPage["interactions"][number] =>
          r.kind === "interaction",
      ),
      surfaces: records.filter((r): r is SurfaceState => r.kind === "surface"),
      ...(offset + count < value.records.length
        ? { next: views.next(id, offset + count, index + 1) }
        : {}),
    };
    try {
      boundedJson(page, limits);
      views.finish(id, !!page.next);
      return ok(page);
    } catch {
      if (count <= 1) {
        views.finish(id, false);
        return fail("limit_exceeded");
      }
      count = Math.floor(count / 2);
    }
  }
}

export function readSessionPage(
  views: ReadViews,
  scope: string,
  query: PageQuery,
  capture: () => Session[],
  limits: Limits,
): Result<SessionPage> {
  const read = views.read(scope, query, capture);
  if (!read.ok) return read;
  const { id, offset, index, value } = read.value;
  if (offset > value.length) return fail("cursor_expired");
  let count = Math.min(query.limit, value.length - offset);
  for (;;) {
    const page: SessionPage = {
      schemaVersion: 2,
      kind: "sessionPage",
      items: value.slice(offset, offset + count),
      ...(offset + count < value.length
        ? { next: views.next(id, offset + count, index + 1) }
        : {}),
    };
    try {
      boundedJson(page, limits);
      views.finish(id, !!page.next);
      return ok(page);
    } catch {
      if (count <= 1) {
        views.finish(id, false);
        return fail("limit_exceeded");
      }
      count = Math.floor(count / 2);
    }
  }
}
