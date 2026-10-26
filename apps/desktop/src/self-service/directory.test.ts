import { expect, it, vi } from "vitest";
import { createDirectory } from "./directory";
import { policyFixturePort } from "../../../../tests/self-service/support";
import type { SelfServicePort } from "./types";
it("advances empty pages without confusing search budgets with a complete directory", async () => {
  const read = vi.fn(async ({ after }) => ({
    items: [],
    nextCursor:
      after == null
        ? "1"
        : Number(after) < 20
          ? String(Number(after) + 1)
          : null,
    asOf: 1000,
  }));
  const c = createDirectory({
    ...policyFixturePort,
    policy_directory: read,
  } as unknown as SelfServicePort);
  await c.load();
  for (let n = 0; n < 20; n++) await c.load(true);
  expect(read).toHaveBeenCalledTimes(21);
  expect(c.state.complete).toBe(true);
  c.dispose();
});
it("does not publish a retired subject's late directory and preserves the retry cursor", async () => {
  let resolve!: (
    v: Awaited<ReturnType<SelfServicePort["policy_directory"]>>,
  ) => void;
  const c = createDirectory({
    ...policyFixturePort,
    policy_directory: () =>
      new Promise((r) => {
        resolve = r;
      }),
  } as unknown as SelfServicePort);
  const pending = c.load();
  c.dispose();
  resolve({ items: [], nextCursor: "old", asOf: 1000 });
  await pending;
  expect(c.state.next).toBeNull();
  expect(c.state.items).toEqual([]);
});

it("starts a fresh discovery after a paged traversal without reusing its visited cursor set", async () => {
  const read = vi
    .fn()
    .mockResolvedValueOnce({ items: [], nextCursor: "1", asOf: 1 })
    .mockResolvedValueOnce({ items: [], nextCursor: "2", asOf: 1 })
    .mockResolvedValueOnce({ items: [], nextCursor: "1", asOf: 2 });
  const c = createDirectory({ ...policyFixturePort, policy_directory: read });
  await c.load();
  await c.load(true);
  await c.load();
  expect(c.state.error).toBe("");
  expect(c.state.next).toBe("1");
  c.dispose();
});
