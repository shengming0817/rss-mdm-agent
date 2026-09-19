import {
  boundedJson,
  type Budget,
  type Failure,
  type Result,
} from "@rss-mdm-agent/ai-contract";
export const limits = {
  maxBytes: 1024 * 1024,
  maxTextBytes: 512 * 1024,
  maxDepth: 32,
  maxNodes: 32768,
};
export const ok = <T>(value: T): Result<T> => ({ ok: true, value });
export const fail = (
  code: Failure["code"],
  retry: Failure["retry"] = "never",
): Result<never> => ({ ok: false, error: { code, retry } });
export const copy = <T>(value: T): T => JSON.parse(boundedJson(value, limits));
/** Internal proof that the fixed native runtime rejected before accepting input. */
export class NativeNotSubmittedError extends Error {
  constructor() {
    super("native request was not submitted");
    this.name = "NativeNotSubmittedError";
  }
}
export function same(a: unknown, b: unknown): boolean {
  const sort = (value: any): any =>
    Array.isArray(value)
      ? value.map(sort)
      : value && typeof value === "object"
        ? Object.fromEntries(
            Object.keys(value)
              .sort()
              .map((k) => [k, sort(value[k])]),
          )
        : value;
  return JSON.stringify(sort(a)) === JSON.stringify(sort(b));
}
export function live(budget: Budget): boolean {
  return (
    !budget.signal.aborted &&
    Number.isSafeInteger(budget.timeoutMs) &&
    budget.timeoutMs > 0 &&
    budget.timeoutMs <= 2147483647
  );
}
export function bounded<T>(work: PromiseLike<T>, budget: Budget): Promise<T> {
  return new Promise((resolve, reject) => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const finish = (f: () => void) => {
      clearTimeout(timer);
      budget.signal.removeEventListener("abort", abort);
      f();
    };
    const abort = () => finish(() => reject(new Error("budget exhausted")));
    Promise.resolve(work).then(
      (v) => finish(() => resolve(v)),
      (error) =>
        finish(() =>
          reject(
            error instanceof NativeNotSubmittedError
              ? error
              : new Error("provider unavailable"),
          ),
        ),
    );
    if (!live(budget)) return abort();
    budget.signal.addEventListener("abort", abort, { once: true });
    timer = setTimeout(abort, budget.timeoutMs);
  });
}
/** One bounded observer; exhausting it stops the incarnation instead of losing stable events. */
export class Queue<T> {
  private items: Array<{ value: T; bytes: number }> = [];
  private bytes = 0;
  private wake?: () => void;
  private ended = false;
  private reading = false;
  push(value: T): void {
    if (this.ended) throw new Error("closed observer");
    const serialized = boundedJson(value, limits),
      bytes = Buffer.byteLength(serialized);
    if (this.items.length >= 1024 || this.bytes + bytes > 4 * 1024 * 1024)
      throw new Error("observer limit");
    this.items.push({ value: JSON.parse(serialized), bytes });
    this.bytes += bytes;
    this.wake?.();
  }
  end(): void {
    this.ended = true;
    this.wake?.();
  }
  async *read(budget: Budget): AsyncGenerator<T> {
    if (this.reading) throw new Error("concurrent observer");
    this.reading = true;
    const deadline = Date.now() + budget.timeoutMs;
    try {
      while (live(budget) && Date.now() < deadline) {
        const item = this.items.shift();
        if (item) {
          this.bytes -= item.bytes;
          yield item.value;
          continue;
        }
        if (this.ended) return;
        try {
          await bounded(
            new Promise<void>((r) => {
              this.wake = r;
            }),
            { ...budget, timeoutMs: deadline - Date.now() },
          );
        } catch {
          return;
        } finally {
          this.wake = undefined;
        }
      }
    } finally {
      this.reading = false;
    }
  }
}
