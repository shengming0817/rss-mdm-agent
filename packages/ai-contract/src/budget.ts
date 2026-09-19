import type { Budget } from "./ports.js";

export type BudgetFactory = () => Budget;
export const defaultBudget: BudgetFactory = () => ({
  timeoutMs: 1000,
  signal: new AbortController().signal,
});

/** Owns a referenced watchdog; AbortSignal alone cannot bound an uncooperative port. */
export async function withinBudget<T>(
  factory: BudgetFactory,
  operation: (budget: Budget) => T | PromiseLike<T>,
  lifetime?: AbortSignal,
): Promise<T> {
  const input = factory();
  if (
    !Number.isSafeInteger(input.timeoutMs) ||
    input.timeoutMs < 1 ||
    input.timeoutMs > 2147483647
  )
    throw new Error("invalid AI operation budget");
  const control = new AbortController();
  const signal = control.signal;
  const sources =
    lifetime && lifetime !== input.signal
      ? [input.signal, lifetime]
      : [input.signal];
  const abort = () => control.abort();
  // ref: nodejs/node#64476. Explicitly own links instead of Node 24 composite dependants.
  for (const source of sources)
    source.addEventListener("abort", abort, { once: true });
  let timer: ReturnType<typeof setTimeout> | undefined;
  let rejectBudget: () => void = () => {};
  const expired = new Promise<never>((_, reject) => {
    rejectBudget = () => reject(new Error("AI operation budget exhausted"));
    signal.addEventListener("abort", rejectBudget, { once: true });
    timer = setTimeout(() => control.abort(), input.timeoutMs);
  });
  try {
    if (sources.some((source) => source.aborted)) control.abort();
    return await Promise.race([
      Promise.resolve().then(() => {
        if (signal.aborted) throw new Error("AI operation budget exhausted");
        return operation({ ...input, signal });
      }),
      expired,
    ]);
  } finally {
    clearTimeout(timer);
    for (const source of sources) source.removeEventListener("abort", abort);
    signal.removeEventListener("abort", rejectBudget);
    control.abort();
  }
}
