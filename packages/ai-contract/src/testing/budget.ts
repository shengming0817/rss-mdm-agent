import type { Budget } from "../ports.js";

export type BudgetFactory = () => Budget;
export const defaultBudget: BudgetFactory = () => ({
  timeoutMs: 1000,
  signal: new AbortController().signal,
});

/** Owns a referenced watchdog; AbortSignal alone cannot bound an uncooperative port. */
export async function withinBudget<T>(
  factory: BudgetFactory,
  operation: (budget: Budget) => T | PromiseLike<T>,
): Promise<T> {
  const input = factory();
  if (
    !Number.isSafeInteger(input.timeoutMs) ||
    input.timeoutMs < 1 ||
    input.timeoutMs > 2147483647
  )
    throw new Error("invalid conformance budget");
  const control = new AbortController();
  const signal = AbortSignal.any([input.signal, control.signal]);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let rejectBudget: () => void = () => {};
  const expired = new Promise<never>((_, reject) => {
    rejectBudget = () => reject(new Error("conformance budget exhausted"));
    signal.addEventListener("abort", rejectBudget, { once: true });
    timer = setTimeout(() => control.abort(), input.timeoutMs);
  });
  try {
    if (signal.aborted) throw new Error("conformance budget exhausted");
    return await Promise.race([
      Promise.resolve().then(() => operation({ ...input, signal })),
      expired,
    ]);
  } finally {
    clearTimeout(timer);
    signal.removeEventListener("abort", rejectBudget);
    control.abort();
  }
}

/** Cleanup failures are retained alongside the original assertion/provider failure. */
export async function withCleanup<T>(
  body: () => Promise<T>,
  cleanup: () => Promise<void>,
): Promise<T> {
  const errors: unknown[] = [];
  let value: T | undefined;
  try {
    value = await body();
  } catch (error) {
    errors.push(error);
  }
  try {
    await cleanup();
  } catch (error) {
    errors.push(error);
  }
  if (errors.length === 1) throw errors[0];
  if (errors.length > 1)
    throw new AggregateError(errors, "conformance and cleanup failed");
  return value as T;
}

/** Public store/Host methods are exercised with an independent watchdog per call.
 * Streaming methods retain their caller signal and bound next/return separately. */
export function boundedPort<T extends object>(
  port: T,
  factory: BudgetFactory,
): T {
  return new Proxy(port, {
    get(target, property) {
      const value = Reflect.get(target, property);
      if (typeof value !== "function") return value;
      if (property === "negotiate") return value.bind(target);
      if (property === "subscribe" || property === "observe")
        return (...args: unknown[]) => ({
          [Symbol.asyncIterator]() {
            const iterator = value.apply(target, args)[Symbol.asyncIterator]();
            return {
              next: () => withinBudget(factory, () => iterator.next()),
              return: () =>
                withinBudget(
                  factory,
                  () => iterator.return?.() ?? { done: true, value: undefined },
                ),
            };
          },
        });
      return (...args: unknown[]) =>
        withinBudget(factory, () => value.apply(target, args));
    },
  });
}

export async function closeAll(
  resources: readonly import("../ports.js").Closeable[],
  factory: BudgetFactory,
): Promise<void> {
  const errors: unknown[] = [];
  for (const resource of [...resources].reverse()) {
    try {
      const result = await withinBudget(factory, (budget) =>
        resource.close(budget),
      );
      if (!result.ok) throw new Error(`cleanup: ${result.error.code}`);
    } catch (error) {
      errors.push(error);
    }
  }
  if (errors.length)
    throw new AggregateError(errors, "resource cleanup failed");
}
