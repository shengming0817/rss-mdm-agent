import { createTaskCenter, taskCenterKey } from "../task-center/controller";
import { taskPort } from "../../../../tests/self-service/support";
import { mount, flushPromises } from "@vue/test-utils";
import { expect, it, vi } from "vitest";
import ConnectionCheck from "./ConnectionCheck.vue";
import type { NetworkCheckPort } from "../self-service/native";
import type { NetworkCheckStatus } from "../self-service/types";
const state = (request: string, phase = "outcomeUnknown") =>
  ({
    status: { operationRequestId: request, phase, cancelRequested: false },
    result: null,
  }) as NetworkCheckStatus;
it("defaults to the backend and submits the edited target only on an explicit click", async () => {
  const call = vi.fn(async (input) =>
    input.kind === "defaults"
      ? { kind: "defaults", value: { host: "mdm.example", port: 443 } }
      : { kind: "status", value: state(input.request) },
  );
  const w = mount(ConnectionCheck, {
    props: { port: call as NetworkCheckPort },
  });
  try {
    await flushPromises();
    expect(call).toHaveBeenCalledTimes(1);
    expect(w.get("input").element.value).toBe("mdm.example");
    await w.get("input").setValue("other.example");
    await w.get("input[type=number]").setValue(8443);
    await w.get("form").trigger("submit");
    await flushPromises();
    const run = call.mock.calls.find(([v]) => v.kind === "run")![0];
    expect(run.target).toEqual({ host: "other.example", port: 8443 });
    const query = w.findAll("button").find((b) => b.text() === "查询原请求")!;
    await query.trigger("click");
    await flushPromises();
    expect(call.mock.calls.at(-1)![0]).toEqual({
      kind: "status",
      request: run.request,
    });
    await w.get("form").trigger("submit");
    expect(call.mock.calls.filter(([v]) => v.kind === "run")).toHaveLength(1);
  } finally {
    w.unmount();
  }
});
it("adopts the exact external target in the original form and allocates its request only on submit", async () => {
  const call = vi.fn(async (input) =>
    input.kind === "defaults"
      ? { kind: "defaults", value: null }
      : { kind: "status", value: state(input.request) },
  );
  const w = mount(ConnectionCheck, {
    props: { port: call as NetworkCheckPort },
  });
  try {
    await flushPromises();
    expect(w.vm.prepareEntry({ host: "external.example", port: 8443 })).toBe(
      true,
    );
    await flushPromises();
    expect(w.get("input").element.value).toBe("external.example");
    expect(call.mock.calls.filter(([v]) => v.kind === "run")).toHaveLength(0);
    await w.get("form").trigger("submit");
    await flushPromises();
    const run = call.mock.calls.find(([v]) => v.kind === "run")![0];
    expect(run.target).toEqual({ host: "external.example", port: 8443 });
    expect(run.request).toMatch(/^[0-9a-f-]{36}$/);
    expect(w.vm.prepareEntry({ host: "other.example", port: 443 })).toBe(false);
  } finally {
    w.unmount();
  }
});
it("lost submission receipts preserve the UUID and cancellation uses it", async () => {
  const call = vi.fn(async (input) => {
    if (input.kind === "defaults") return { kind: "defaults", value: null };
    if (input.kind === "run") throw Error("lost");
    return { kind: "status", value: state(input.request) };
  });
  const owner = taskPort();
  owner.control = vi.fn(async (input) => ({
    id: input.id,
    disposition: "confirmed" as const,
    created: null,
  }));
  const tasks = createTaskCenter(owner, null);
  const w = mount(ConnectionCheck, {
    props: { port: call as NetworkCheckPort },
    global: { provide: { [taskCenterKey as symbol]: tasks } },
  });
  try {
    await flushPromises();
    await w.get("input").setValue("127.0.0.1");
    await w.get("form").trigger("submit");
    await flushPromises();
    const original = call.mock.calls.find(([v]) => v.kind === "run")![0]
      .request;
    await w
      .findAll("button")
      .find((b) => b.text() === "查询原请求")!
      .trigger("click");
    await flushPromises();
    await w
      .findAll("button")
      .find((b) => b.text() === "取消检查")!
      .trigger("click");
    await flushPromises();
    expect(owner.control).toHaveBeenCalledWith(
      expect.objectContaining({
        reference: { kind: "request", request: original },
        action: "cancel",
      }),
    );
    expect(call.mock.calls.at(-1)![0]).toEqual({
      kind: "status",
      request: original,
    });
  } finally {
    w.unmount();
    tasks.dispose();
  }
});
it("missing native capability is explicit and cannot simulate a result", async () => {
  const w = mount(ConnectionCheck, { props: { port: null } });
  expect(w.text()).toContain("无法运行检查");
  expect(w.get("button").attributes("disabled")).toBeDefined();
  w.unmount();
});
