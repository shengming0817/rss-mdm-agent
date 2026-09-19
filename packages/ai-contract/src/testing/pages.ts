import type { Clock, Result } from "../ports.js";
import type { PageQuery } from "../wire.js";

/** Bounded immutable read views for the memory conformance double, not durable storage. */
export class ReadViews {
  private views = new Map<
    string,
    {
      scope: string;
      expires: number;
      limit: number;
      value: unknown;
      continued: boolean;
    }
  >();
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
      if (view.expires < now) this.views.delete(id);
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
    this.views.set(id, {
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
      this.views.delete(id);
      for (const [token, row] of this.tokens)
        if (row.id === id) this.tokens.delete(token);
    }
  }
  clear(): void {
    this.views.clear();
    this.tokens.clear();
  }
}
