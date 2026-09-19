import {
  boundedJson,
  type Budget,
  type Failure,
  type Limits,
  type Result,
} from "@rss-mdm-agent/ai-contract";
export const limits: Limits = {
  maxBytes: 262144,
  maxTextBytes: 131072,
  maxDepth: 24,
  maxNodes: 8192,
};
export const ok = <T>(value: T): Result<T> => ({ ok: true, value });
export const fail = (
  code: Failure["code"],
  retry: Failure["retry"] = "never",
): Result<never> => ({ ok: false, error: { code, retry } });
export const copy = <T>(value: T): T => JSON.parse(boundedJson(value, limits));
export const id = (value: unknown): value is string =>
  typeof value === "string" &&
  /^[A-Za-z0-9][A-Za-z0-9._:/+\-]{0,127}$/.test(value);
export function same(a: unknown, b: unknown): boolean {
  const sort = (v: any): any =>
    Array.isArray(v)
      ? v.map(sort)
      : v && typeof v === "object"
        ? Object.fromEntries(
            Object.keys(v)
              .sort()
              .map((k) => [k, sort(v[k])]),
          )
        : v;
  try {
    return JSON.stringify(sort(copy(a))) === JSON.stringify(sort(copy(b)));
  } catch {
    return false;
  }
}
export function bounded<T>(work: PromiseLike<T>, budget: Budget): Promise<T> {
  return new Promise((resolve, reject) => {
    const abort = () => finish(() => reject(new Error("budget exhausted")));
    let timer: ReturnType<typeof setTimeout> | undefined;
    const finish = (f: () => void) => {
      clearTimeout(timer);
      budget.signal.removeEventListener("abort", abort);
      f();
    };
    Promise.resolve(work).then(
      (v) => finish(() => resolve(v)),
      () => finish(() => reject(new Error("provider unavailable"))),
    );
    if (
      !Number.isSafeInteger(budget.timeoutMs) ||
      budget.timeoutMs <= 0 ||
      budget.signal.aborted
    )
      return abort();
    budget.signal.addEventListener("abort", abort, { once: true });
    timer = setTimeout(abort, budget.timeoutMs);
  });
}
export function deferred<T>() {
  let resolve!: (v: T) => void;
  const promise = new Promise<T>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}
/** Single consumer queue. Abort only detaches the current observer. */
export class Queue<T> implements AsyncIterable<T> {
  private items: T[] = [];
  private wake?: () => void;
  private ended = false;
  private bytes = 0;
  constructor(
    private readonly maxItems = 1024,
    private readonly maxBytes = 2 * 1024 * 1024,
  ) {}
  push(value: T): void {
    if (this.ended) throw new Error("closed queue");
    const size = Buffer.byteLength(boundedJson(value, limits));
    if (this.items.length >= this.maxItems || this.bytes + size > this.maxBytes)
      throw new Error("queue limit");
    this.items.push(copy(value));
    this.bytes += size;
    this.wake?.();
  }
  end(): void {
    this.ended = true;
    this.wake?.();
  }
  async *read(budget?: Budget): AsyncGenerator<T> {
    if (this.wake) throw new Error("concurrent observer");
    const deadline = budget ? Date.now() + budget.timeoutMs : Infinity;
    while (true) {
      if (budget?.signal.aborted || Date.now() >= deadline) return;
      if (this.items.length) {
        const item = this.items.shift()!;
        this.bytes -= Buffer.byteLength(boundedJson(item, limits));
        yield item;
        continue;
      }
      if (this.ended) return;
      const ready = new Promise<void>((r) => {
        this.wake = r;
      });
      try {
        if (budget)
          await bounded(ready, { ...budget, timeoutMs: deadline - Date.now() });
        else await ready;
      } catch {
        return;
      } finally {
        this.wake = undefined;
      }
    }
  }
  [Symbol.asyncIterator](): AsyncIterator<T> {
    return this.read();
  }
}
