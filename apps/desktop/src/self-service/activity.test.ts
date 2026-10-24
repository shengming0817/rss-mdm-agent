import { expect, it, vi } from "vitest";
import { createActivity } from "./activity";
import {
  executionTask,
  policyFixturePort,
} from "../../../../tests/self-service/support";
import type { SelfServicePort } from "./types";
it("continues an empty authorized page, de-duplicates original requests and discards retired replies", async () => {
  const recent = vi
    .fn()
    .mockResolvedValueOnce({ items: [], next: "10" })
    .mockResolvedValueOnce({
      items: [{ details: executionTask(), recordedAtUnixMs: 1000 }],
      next: "5",
    })
    .mockResolvedValueOnce({
      items: [{ details: executionTask(), recordedAtUnixMs: 900 }],
      next: null,
    });
  const c = createActivity({
    ...policyFixturePort,
    recent_tasks: recent,
  } as unknown as SelfServicePort);
  await c.load();
  expect(c.state.loaded).toBe(true);
  expect(c.state.next).toBe("10");
  await c.load(true);
  await c.load(true);
  expect(c.state.items).toHaveLength(1);
  expect(c.state.items[0].recordedAtUnixMs).toBe(1000);
  let resolve!: (value: unknown) => void;
  recent.mockImplementationOnce(
    () =>
      new Promise((r) => {
        resolve = r;
      }),
  );
  const pending = c.load();
  c.dispose();
  resolve({
    items: [{ details: executionTask(), recordedAtUnixMs: 2000 }],
    next: null,
  });
  await pending;
  expect(c.state.items).toEqual([]);
});
it("retains failed page cursors and detects a repeated continuation", async () => {
  const recent = vi.fn().mockResolvedValue({ items: [], next: "10" });
  const c = createActivity({
    recent_tasks: recent,
  } as unknown as unknown as SelfServicePort);
  await c.load();
  await c.load(true);
  expect(c.state.error).toContain("未完成");
  expect(c.state.next).toBe("10");
  c.dispose();
});
