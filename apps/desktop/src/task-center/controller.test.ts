import { expect, it, vi } from "vitest";
import { createTaskCenter } from "./controller";
import {
  taskPort,
  taskPage,
  taskDetail,
  policyFixturePort,
  offer,
} from "../../../../tests/self-service/support";
it("coalesces list and exact detail reads and refreshes browsed pages with fresh cursors", async () => {
  const port = taskPort();
  let finish!: (v: ReturnType<typeof taskPage>) => void;
  port.list = vi.fn(
    () =>
      new Promise<ReturnType<typeof taskPage>>((resolve) => {
        finish = resolve;
      }),
  );
  const c = createTaskCenter(port, null),
    a = c.load(),
    b = c.load();
  expect(port.list).toHaveBeenCalledTimes(1);
  finish(taskPage([taskDetail()]));
  await Promise.all([a, b]);
  expect(c.state.items).toHaveLength(1);
  c.dispose();
});
it("queries the original control receipt after reply loss, without submitting another control", async () => {
  const port = taskPort(),
    item = taskDetail();
  port.control = vi.fn(async () => {
    throw Error("lost");
  });
  port.controlStatus = vi.fn(async (q) => ({
    id: q.id,
    disposition: "confirmed" as const,
    created: null,
  }));
  const c = createTaskCenter(port, null);
  await c.openTask(item.reference);
  await c.control(item, "cancel");
  expect(c.state.uncertain).toBe(true);
  const original = c.state.pendingControl!;
  await c.control(item, "cancel");
  expect(port.control).toHaveBeenCalledTimes(1);
  await c.reconcileControl();
  expect(port.controlStatus).toHaveBeenCalledWith({
    reference: original.reference,
    id: original.id,
    scope: original.scope,
  });
  expect(c.state.pendingControl).toBeNull();
  c.dispose();
});
it("rejects old-scope and late detail responses and clears allowed actions", async () => {
  const port = taskPort(),
    c = createTaskCenter(port, null);
  await c.load();
  let resolve!: (v: ReturnType<typeof taskDetail>) => void;
  port.detail = vi.fn(
    () =>
      new Promise<ReturnType<typeof taskDetail>>((r) => {
        resolve = r;
      }),
  );
  const pending = c.openTask(taskDetail().reference);
  c.dispose();
  resolve(taskDetail());
  await pending;
  expect(c.state.items).toEqual([]);
  expect(c.state.selected).toBeNull();
});
it("hidden and disposed consumers stop refreshing without cancelling original tasks", async () => {
  vi.useFakeTimers();
  const port = taskPort();
  port.list = vi.fn(async () => taskPage([taskDetail()]));
  port.control = vi.fn();
  const c = createTaskCenter(port, null);
  c.setVisible(true);
  await vi.advanceTimersByTimeAsync(0);
  const count = vi.mocked(port.list).mock.calls.length;
  c.setVisible(false);
  await vi.advanceTimersByTimeAsync(60000);
  expect(port.list).toHaveBeenCalledTimes(count);
  expect(port.control).not.toHaveBeenCalled();
  c.dispose();
  vi.useRealTimers();
});
it("deduplicates trusted human confirmation and recovers the original request without executing again", async () => {
  const port = taskPort(),
    business = {
      ...policyFixturePort,
      offers: vi.fn(async () => []),
      execute: vi.fn(),
      confirm: vi.fn(async () => {
        throw Error("lost");
      }),
    };
  const c = createTaskCenter(port, business);
  await Promise.all([
    c.confirmPreparation(offer()),
    c.confirmPreparation(offer()),
  ]);
  expect(business.confirm).toHaveBeenCalledTimes(1);
  expect(business.execute).not.toHaveBeenCalled();
  expect(c.state.selected?.reference).toEqual({
    kind: "request",
    request: offer().request,
  });
  c.dispose();
});
it("clears a definitively rejected control but retains Unknown and never resubmits it", async () => {
  const port = taskPort(),
    item = taskDetail(),
    c = createTaskCenter(port, null);
  port.control = vi.fn(async () => {
    throw { code: "task_stale" };
  });
  await c.openTask(item.reference);
  await c.control(item, "cancel");
  expect(c.state.pendingControl).toBeNull();
  expect(c.state.uncertain).toBe(false);
  port.control = vi.fn(async (input) => ({
    id: input.id,
    disposition: "unknown" as const,
    created: null,
  }));
  await c.control(item, "cancel");
  await c.control(item, "cancel");
  expect(port.control).toHaveBeenCalledTimes(1);
  expect(c.state.pendingControl).not.toBeNull();
  c.dispose();
});
it("fences a late detail response after a native scope change and drops old actions", async () => {
  const port = taskPort(),
    c = createTaskCenter(port, null);
  await c.openTask(taskDetail().reference);
  let finish!: (v: ReturnType<typeof taskDetail>) => void;
  port.detail = vi.fn(
    () =>
      new Promise<ReturnType<typeof taskDetail>>((r) => {
        finish = r;
      }),
  );
  const pending = c.openTask({ kind: "request", request: "late" });
  port.list = vi.fn(async () => ({ ...taskPage(), scope: "new-scope" }));
  await c.load();
  finish(taskDetail("late"));
  await pending;
  expect(c.state.selected).toBeNull();
  expect(c.state.byRef.size).toBe(0);
  expect(c.state.pendingControl).toBeNull();
  c.dispose();
});
it("keeps paging beyond the cache budget and preserves the current page across detail navigation", async () => {
  const port = taskPort();
  port.list = vi.fn(async (query) => {
    const index = Number(query.cursor ?? 0);
    return {
      ...taskPage(
        Array.from({ length: 32 }, (_, n) => taskDetail(`page-${index}-${n}`)),
      ),
      next: index < 18 ? String(index + 1) : null,
    };
  });
  const c = createTaskCenter(port, null);
  await c.load();
  for (let i = 0; i < 17; i++) await c.load(true);
  expect(c.state.page).toBe(18);
  expect(c.state.items).toHaveLength(32);
  expect(c.state.items[0].reference).toEqual({
    kind: "request",
    request: "page-17-0",
  });
  expect(c.state.next).toBe("18");
  expect(c.state.byRef.size).toBeLessThanOrEqual(512);
  await c.openTask(c.state.items[0].reference);
  await c.previousPage();
  expect(c.state.page).toBe(17);
  expect(c.state.next).toBe("17");
  c.dispose();
});
it("removes a denied original task from cached consumers and drops stale step actions when its owner advances", async () => {
  const port = taskPort(),
    item = taskDetail(),
    c = createTaskCenter(port, null);
  port.list = vi.fn(async () => taskPage([item]));
  await c.load();
  await c.openTask(item.reference);
  port.detail = vi.fn(async () => {
    throw { code: "task_denied" };
  });
  await c.openTask(item.reference);
  expect(c.lookup(item.reference)).toBeUndefined();
  expect(c.state.items).toEqual([]);
  expect(c.state.selected).toBeNull();
  port.detail = vi.fn(async () => ({
    ...item,
    stage: "verified",
    allowedActions: [],
  }));
  expect(await c.openNode(item.reference, "execution/pending")).toBe(
    "unavailable",
  );
  expect(c.state.selected?.stage).toBe("verified");
  expect(c.state.selected?.allowedActions).toEqual([]);
  expect(c.state.node).toBe("");
  c.dispose();
});
