import { withinBudget, type BudgetFactory } from "../budget.js";
export { withinBudget, defaultBudget, type BudgetFactory } from "../budget.js";

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
