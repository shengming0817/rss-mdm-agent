import { describe, expect, it, vi } from "vitest";
import { createController } from "./controller";
import {
  executionTask,
  snapshot,
} from "../../../../tests/self-service/support";
import type { SelfServicePort } from "./types";
function fixture() {
  const value = snapshot();
  const port = {
    snapshot: vi.fn<SelfServicePort["snapshot"]>(async () =>
      structuredClone(value),
    ),
    execute: vi.fn<SelfServicePort["execute"]>(async (input) => ({
      request: input.request,
      confirmationRequired: false,
    })),
    cancel: vi.fn<SelfServicePort["cancel"]>(async () => ({
      kind: "execution",
      value: executionTask(),
    })),
  } satisfies SelfServicePort;
  return {
    value,
    port,
    c: createController(port, {
      available: [],
      preparations: [],
      selected: null,
      requests: [],
      next: null,
    }),
  };
}
describe("backend task selection", () => {
  it("requires confirmation and sends only the exact offered references", async () => {
    const { c, port, value } = fixture();
    await c.refresh();
    c.select(value.available[0]!);
    expect(port.execute).not.toHaveBeenCalled();
    await Promise.all([c.confirm(), c.confirm()]);
    expect(port.execute).toHaveBeenCalledTimes(1);
    expect(port.execute).toHaveBeenCalledWith({
      task: "backend-task",
      attempt: "backend-attempt",
      request: "backend-request",
      revision: "a".repeat(64),
    });
    expect(c.state.taskId).toBe("backend-request");
  });
  it("keeps an ambiguous original submission and does not create a replacement", async () => {
    const { c, port, value } = fixture();
    port.execute.mockRejectedValueOnce(new Error("lost reply"));
    await c.refresh();
    c.select(value.available[0]!);
    await c.confirm();
    c.select({ ...value.available[0]!, attempt: "other-attempt" });
    await c.confirm();
    expect(c.state.uncertain).toBe(true);
    expect(port.execute).toHaveBeenCalledTimes(1);
    expect(c.state.taskId).toBe("backend-request");
  });
  it("withdraws a stale offer instead of changing the selected revision", async () => {
    const { c, value, port } = fixture();
    await c.refresh();
    c.select(value.available[0]!);
    value.available[0]!.revision = "b".repeat(64);
    await c.refresh();
    await c.confirm();
    expect(c.state.item).toBeNull();
    expect(port.execute).not.toHaveBeenCalled();
  });
  it("failure does not install a fixture fallback", async () => {
    const { c, port } = fixture();
    port.snapshot.mockRejectedValueOnce(new Error("offline"));
    await c.refresh();
    expect(c.state.snapshot).toBeNull();
    expect(c.state.error).toContain("原任务保留");
  });
  it("late replies cannot enter a disposed user view", async () => {
    const { c, port, value } = fixture();
    let resolve!: (v: typeof value) => void;
    port.snapshot.mockImplementationOnce(
      () =>
        new Promise((r) => {
          resolve = r;
        }),
    );
    const pending = c.refresh();
    c.dispose();
    resolve(value);
    await pending;
    expect(c.state.snapshot).toBeNull();
  });
});

it("keeps the selected task query independent of paging and returns to the first page", async () => {
  const { c, port, value } = fixture();
  value.next = "page-2";
  await c.refresh();
  c.select(value.available[0]!);
  await c.next();
  expect(port.snapshot).toHaveBeenLastCalledWith({
    after: "page-2",
    selected: "backend-request",
  });
  await c.previous();
  expect(port.snapshot).toHaveBeenLastCalledWith({
    after: null,
    selected: "backend-request",
  });
  await c.next();
  await c.first();
  expect(c.state.previous).toEqual([]);
  expect(c.state.after).toBeNull();
});
