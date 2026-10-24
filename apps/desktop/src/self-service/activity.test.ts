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

it("refreshes the browsed page count with fresh cursors, including empty authorized pages", async () => {
  const recent = vi
    .fn()
    .mockResolvedValueOnce({ items: [], next: "10" })
    .mockResolvedValueOnce({ items: [], next: "5" })
    .mockResolvedValueOnce({ items: [], next: "11" })
    .mockResolvedValueOnce({ items: [], next: "6" });
  const c = createActivity({ recent_tasks: recent });
  await c.load();
  await c.load(true);
  await c.load();
  expect(c.state.error).toBe("");
  expect(c.state.next).toBe("6");
  expect(recent.mock.calls).toEqual([
    [{ before: null }],
    [{ before: "10" }],
    [{ before: null }],
    [{ before: "11" }],
  ]);
  c.dispose();
});

it("retains all browsed records and the cursor when a later refresh page fails", async () => {
  const task = (id: string, phase = "running") => {
    const details = executionTask();
    details.action.requestId = id;
    details.status.phase = phase as typeof details.status.phase;
    return { details, recordedAtUnixMs: 1000 };
  };
  const recent = vi
    .fn()
    .mockResolvedValueOnce({ items: [task("first")], next: "10" })
    .mockResolvedValueOnce({ items: [task("second")], next: "5" })
    .mockResolvedValueOnce({ items: [task("new")], next: "11" })
    .mockRejectedValueOnce(Error("offline"));
  const c = createActivity({ recent_tasks: recent });
  await c.load();
  await c.load(true);
  await c.load();
  expect(c.state.items.map((item) => item.details.action.requestId)).toEqual([
    "first",
    "second",
  ]);
  expect(c.state.next).toBe("5");
  expect(c.state.error).toContain("未完成");
  recent
    .mockResolvedValueOnce({ items: [task("new")], next: "12" })
    .mockResolvedValueOnce({ items: [task("first", "verified")], next: null });
  await c.load();
  expect(c.state.items.map((item) => item.details.action.requestId)).toEqual([
    "new",
    "first",
  ]);
  expect(c.state.items[1].details.status.phase).toBe("verified");
  expect(c.state.next).toBeNull();
  expect(c.state.error).toBe("");
  c.dispose();
});
