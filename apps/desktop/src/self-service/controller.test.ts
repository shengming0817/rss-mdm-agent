import { mount, flushPromises } from "@vue/test-utils";
import SelfService from "./SelfService.vue";
import { describe, expect, it, vi } from "vitest";
import { createController } from "./controller";
import preview from "./preview";
import { executionTask } from "./testing";
import type { SelfServicePort } from "./types";
function fixture() {
  const snapshot = structuredClone(preview);
  const task = executionTask();
  const port = {
    snapshot: vi.fn<SelfServicePort["snapshot"]>(async () =>
      structuredClone(snapshot),
    ),
    execute: vi.fn<SelfServicePort["execute"]>(async (input) => ({
      ...structuredClone(task),
      action: { ...task.action, requestId: input.requestId },
    })),
    confirm: vi.fn<SelfServicePort["confirm"]>(async () => ({
      ...structuredClone(task),
      status: "waiting",
    })),
    cancel: vi.fn<SelfServicePort["cancel"]>(async () => ({
      ...structuredClone(task),
      status: "stopped",
    })),
  } satisfies SelfServicePort;
  let ids = 0;
  const c = createController(
    port,
    () => `request-${++ids}`,
    structuredClone(preview),
  );
  return { c, port, snapshot, task };
}
describe("single execution request", () => {
  it("accepts once and exposes the same action for confirmation", async () => {
    const { c, port, snapshot } = fixture();
    await c.refresh();
    c.select(snapshot.catalog[0]!);
    await c.execute();
    await c.execute();
    expect(port.execute).toHaveBeenCalledTimes(1);
    expect(c.state.page).toBe("tasks");
    expect(c.state.accepted).toBe(true);
    expect(c.state.snapshot!.referencedRequests[0]!.status).toBe(
      "confirmation",
    );
  });
  it("retains exact input and ID across a lost execute response", async () => {
    const { c, port, snapshot } = fixture();
    await c.refresh();
    c.select(snapshot.catalog[0]!);
    c.change("x", { kind: "text", value: "original" });
    port.execute.mockRejectedValueOnce({
      code: "outcomeUnknown",
      message: "unknown",
    });
    await c.execute();
    expect(c.state.uncertain).toBe(true);
    c.change("x", { kind: "text", value: "changed" });
    c.select(snapshot.catalog[1]!, true);
    await c.execute();
    expect(port.execute.mock.calls[0]![0]).toEqual(
      port.execute.mock.calls[1]![0],
    );
  });
  it("queries an uncertain request without creating another", async () => {
    const { c, port, snapshot, task } = fixture();
    await c.refresh();
    c.select(snapshot.catalog[0]!);
    port.execute.mockRejectedValueOnce(new Error());
    await c.execute();
    snapshot.referencedRequests = [
      { ...task, action: { ...task.action, requestId: c.state.requestId } },
    ];
    await c.refresh();
    await c.execute();
    expect(port.execute).toHaveBeenCalledTimes(1);
    expect(c.state.uncertain).toBe(false);
  });
  it("explicit new action gets a new identity after editing", async () => {
    const { c, snapshot } = fixture();
    await c.refresh();
    c.select(snapshot.catalog[0]!);
    const first = c.state.requestId;
    c.change("x", { kind: "text", value: "new" });
    expect(c.state.requestId).not.toBe(first);
  });
  it.each(["confirm", "cancel"] as const)(
    "replays the same %s after response loss",
    async (kind) => {
      const { c, port, task } = fixture();
      await c.refresh();
      port[kind].mockRejectedValueOnce(new Error());
      await c[kind](task);
      await c[kind](task);
      expect(port[kind].mock.calls[0]).toEqual(port[kind].mock.calls[1]);
      expect(port.execute).not.toHaveBeenCalled();
    },
  );
  it("never confirms a non-confirmation task", async () => {
    const { c, port, task } = fixture();
    await c.refresh();
    task.status = "stopped";
    await c.confirm(task);
    expect(port.confirm).not.toHaveBeenCalled();
  });
  it("rejects cross-action retry while an action outcome is unknown", async () => {
    const { c, port, task } = fixture();
    await c.refresh();
    port.confirm.mockRejectedValueOnce(new Error());
    await c.confirm(task);
    await c.confirm({
      ...task,
      action: { ...task.action, digest: "b".repeat(64) },
    });
    expect(port.confirm).toHaveBeenCalledTimes(1);
  });
  it("does not fall back to browser success when snapshot fails", async () => {
    const { c, port } = fixture();
    port.snapshot.mockRejectedValueOnce(new Error());
    await c.refresh();
    expect(c.state.snapshot).toBeNull();
    expect(c.state.error).not.toBe("");
  });
  it("invalidates a selected catalogue entry withdrawn by the service", async () => {
    const { c, port, snapshot } = fixture();
    await c.refresh();
    c.select(snapshot.catalog[0]!);
    snapshot.catalog[0]!.availability = "withdrawn";
    await c.refresh();
    await c.execute();
    expect(port.execute).not.toHaveBeenCalled();
  });
  it("refreshes selected off-page request and preserves pagination", async () => {
    const { c, port, snapshot, task } = fixture();
    snapshot.next = "next-page";
    await c.refresh();
    c.state.taskId = task.action.requestId;
    await c.nextPage();
    expect(port.snapshot.mock.calls.at(-1)![0]).toEqual({
      after: "next-page",
      requestIds: [task.action.requestId],
    });
    await c.previousPage();
    expect(port.snapshot.mock.calls.at(-1)![0].after).toBeNull();
  });
  it("stale snapshot cannot replace a newly returned execution", async () => {
    const { c, port, snapshot } = fixture();
    await c.refresh();
    c.select(snapshot.catalog[0]!);
    let finish!: (v: typeof snapshot) => void;
    port.snapshot.mockImplementationOnce(
      () =>
        new Promise((r) => {
          finish = r;
        }),
    );
    const refresh = c.refresh();
    await c.execute();
    finish(structuredClone(snapshot));
    await refresh;
    expect(c.state.snapshot!.referencedRequests).toHaveLength(1);
  });
});

it("a lost confirmation response followed by Unknown unlocks cancellation of the original request", async () => {
  const { c, port, snapshot, task } = fixture();
  await c.refresh();
  port.confirm.mockRejectedValueOnce(new Error());
  await c.confirm(task);
  snapshot.referencedRequests = [{ ...task, status: "unknownEffect" }];
  await c.refresh();
  await c.cancel(snapshot.referencedRequests[0]!);
  expect(port.cancel).toHaveBeenCalledTimes(1);
  expect(port.execute).not.toHaveBeenCalled();
});

it("ParameterForm preserves exact numeric tokens, explicit false and secret references in execute", async () => {
  const { c, port, snapshot } = fixture();
  await c.refresh();
  c.select(snapshot.catalog.find((i) => i.itemId === "diagnostics")!);
  const wrapper = mount(SelfService, { props: { controller: c } });
  await flushPromises();
  await wrapper.get("#draft-host").setValue("example.invalid");
  await wrapper.get("#draft-count").setValue("3.0000000000000000001");
  await wrapper.get("#draft-detail").setValue("false");
  expect(wrapper.get("#draft-credential").attributes("type")).toBe("password");
  expect(wrapper.get("#draft-credential-revision").attributes("type")).toBe(
    "password",
  );
  await wrapper.get("#draft-credential").setValue("secret-id");
  await wrapper.get("#draft-credential-revision").setValue("r2");
  await wrapper.get("form").trigger("submit");
  await flushPromises();
  expect(port.execute.mock.calls[0]![0].fields).toEqual({
    host: { kind: "text", value: "example.invalid" },
    count: { kind: "integer", value: "3.0000000000000000001" },
    detail: { kind: "boolean", value: false },
    credential: { kind: "secretReference", id: "secret-id", revision: "r2" },
  });
  wrapper.unmount();
});
it("ParameterForm leaves defaulted fields absent instead of materializing defaults", async () => {
  const { c, port, snapshot } = fixture();
  await c.refresh();
  c.select(snapshot.catalog.find((i) => i.itemId === "diagnostics")!);
  const wrapper = mount(SelfService, { props: { controller: c } });
  await flushPromises();
  await wrapper.get("#draft-host").setValue("example.invalid");
  await wrapper.get("form").trigger("submit");
  await flushPromises();
  expect(port.execute.mock.calls[0]![0].fields).toEqual({
    host: { kind: "text", value: "example.invalid" },
  });
  wrapper.unmount();
});
