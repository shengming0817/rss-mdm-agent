import { expect, it, vi } from "vitest";
import { createEntryNavigation } from "./entry-controller";
import type { NavigationPort, NavigationOutput } from "./entry";
const intent = {
  action: "view" as const,
  selector: { kind: "node" as const, id: "home" },
  namespace: null,
  version: null,
  revision: null,
  step: null,
  parameters: {},
};
const deferred = <T>() => {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((r) => {
    resolve = r;
  });
  return { promise, resolve };
};
it("unsubscribes a listener acquired after owner retirement without reading or opening old entries", async () => {
  const registered = deferred<() => void>(),
    stop = vi.fn(),
    port = vi.fn<NavigationPort>(),
    open = vi.fn(async () => true),
    owner = createEntryNavigation(port, () => registered.promise, open),
    starting = owner.start();
  owner.dispose();
  registered.resolve(stop);
  await starting;
  expect(stop).toHaveBeenCalledOnce();
  expect(port).not.toHaveBeenCalled();
  expect(open).not.toHaveBeenCalled();
});
it("suppresses late resolved targets after identity retirement", async () => {
  const resolved = deferred<NavigationOutput>(),
    open = vi.fn(async () => true),
    port = vi.fn<NavigationPort>(async (input) =>
      input.kind === "next"
        ? { kind: "pending", value: { id: "original", intent } }
        : resolved.promise,
    ),
    owner = createEntryNavigation(port, async () => () => {}, open),
    starting = owner.start();
  await vi.waitFor(() =>
    expect(port).toHaveBeenCalledWith({ kind: "resolve", intent }),
  );
  owner.dispose();
  resolved.resolve({ kind: "event", value: intent });
  await starting;
  expect(open).not.toHaveBeenCalled();
  expect(
    port.mock.calls.some(
      ([input]) => input.kind === "ack" || input.kind === "submit",
    ),
  ).toBe(false);
});
it("keeps unsupported deliveries pending until explicitly dismissed and never submits an entry", async () => {
  let pending = true;
  const port = vi.fn<NavigationPort>(async (input) => {
    if (input.kind === "next")
      return {
        kind: "pending",
        value: pending ? { id: "original", intent } : null,
      };
    if (input.kind === "resolve") return { kind: "event", value: intent };
    if (input.kind === "ack") {
      expect(input.id).toBe("original");
      pending = false;
      return { kind: "done" };
    }
    throw Error("unexpected execution");
  });
  const owner = createEntryNavigation(
    port,
    async () => () => {},
    async () => false,
  );
  await owner.start();
  expect(owner.state.pending).toBe("original");
  expect(owner.state.notice).toContain("未执行");
  expect(port.mock.calls.some(([input]) => input.kind === "ack")).toBe(false);
  await owner.dismiss();
  expect(pending).toBe(false);
  expect(owner.state.pending).toBe("");
  expect(port.mock.calls.some(([input]) => input.kind === "submit")).toBe(
    false,
  );
  owner.dispose();
});
it("coalesces native notifications while opening a target and acknowledges only the original delivery", async () => {
  let notify = () => {},
    pending = true;
  const opened = deferred<boolean>(),
    open = vi.fn(() => opened.promise),
    port = vi.fn<NavigationPort>(async (input) => {
      if (input.kind === "next")
        return {
          kind: "pending",
          value: pending ? { id: "original", intent } : null,
        };
      if (input.kind === "resolve") return { kind: "event", value: intent };
      if (input.kind === "ack") {
        pending = false;
        return { kind: "done" };
      }
      throw Error("unexpected");
    }),
    owner = createEntryNavigation(
      port,
      async (listener) => {
        notify = listener;
        return () => {};
      },
      open,
    ),
    starting = owner.start();
  await vi.waitFor(() => expect(open).toHaveBeenCalledOnce());
  notify();
  notify();
  notify();
  opened.resolve(true);
  await starting;
  expect(open).toHaveBeenCalledOnce();
  expect(port.mock.calls.filter(([input]) => input.kind === "ack")).toEqual([
    [{ kind: "ack", id: "original" }],
  ]);
  owner.dispose();
});
