import { describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createController } from "./controller";
import preview from "./preview";
import type { Plan, RequestView, SelfServicePort } from "./types";
import SelfService from "./SelfService.vue";
import ParameterForm from "./ParameterForm.vue";
function fixture() {
  const snapshot = structuredClone(preview);
  const task = structuredClone(
    snapshot.requests.find((r) => r.plan.itemId === "diagnostics")!,
  );
  snapshot.requests = [];
  const port = {
    snapshot: vi.fn<SelfServicePort["snapshot"]>(async () =>
      structuredClone(snapshot),
    ),
    preview: vi.fn<SelfServicePort["preview"]>(async (input) => ({
      ...structuredClone(task.plan),
      requestId: input.requestId,
      revision: input.revision,
    })),
    submit: vi.fn<SelfServicePort["submit"]>(async (input) => ({
      ...structuredClone(task),
      plan: {
        ...task.plan,
        requestId: input.requestId,
        planId: input.planId,
        digest: input.digest,
      },
    })),
    respond: vi.fn<SelfServicePort["respond"]>(async () =>
      structuredClone(task),
    ),
  } satisfies SelfServicePort;
  let ids = 0;
  const c = createController(
    port,
    () => `request-${++ids}`,
    structuredClone(preview),
  );
  return { c, port, snapshot, task };
}
describe("self-service controller", () => {
  it("shows the frozen action in both preview and task details", async () => {
    const { c, snapshot } = fixture();
    const wrapper = mount(SelfService, { props: { controller: c } });
    await flushPromises();
    c.select(snapshot.catalog.find((i) => i.itemId === "diagnostics")!);
    await c.prepare();
    await flushPromises();
    expect(wrapper.get(".plan-summary").text()).toContain(c.state.plan!.action);
    expect(wrapper.get(".plan-summary").text()).toContain("操作");
    await c.submit();
    await flushPromises();
    expect(wrapper.get(".plan-summary").text()).toContain(c.state.plan!.action);
    expect(wrapper.get(".plan-summary").text()).toContain("操作");
    wrapper.unmount();
  });
  it("keeps a usable page when the service restarts during item details", async () => {
    const { c, snapshot } = fixture();
    const wrapper = mount(SelfService, { props: { controller: c } });
    await flushPromises();
    c.select(snapshot.catalog[0]);
    snapshot.instanceId = "restarted";
    await c.refresh();
    await flushPromises();
    expect(c.state.page).toBe("home");
    expect(wrapper.text()).toContain("浏览软件");
    expect(c.state.item).toBeNull();
    wrapper.unmount();
  });
  it.each(["expired", "withdrawn", "missing", "revision", "variant"])(
    "rebinds details and invalidates a preview when catalog becomes %s",
    async (change) => {
      const { c, snapshot } = fixture();
      const wrapper = mount(SelfService, { props: { controller: c } });
      await flushPromises();
      const item = snapshot.catalog.find((i) => i.itemId === "diagnostics")!;
      c.select(
        c.state.snapshot!.catalog.find((i) => i.itemId === "diagnostics")!,
      );
      await c.prepare();
      if (change === "missing") snapshot.catalog = [];
      else if (change === "revision") item.catalog.identity.revision = "r2";
      else if (change === "variant") item.variantId = "another-variant";
      else {
        item.availability = change as "expired" | "withdrawn";
        item.display.requestability = "unknown";
        item.reason = "刷新后的不可申请原因";
      }
      await c.refresh();
      await flushPromises();
      expect(c.state.plan).toBeNull();
      if (["missing", "revision", "variant"].includes(change)) {
        expect(c.state.item).toBeNull();
        expect(c.state.page).toBe("tools");
      } else {
        expect(c.state.item?.availability).toBe(change);
        expect(wrapper.text()).toContain("刷新后的不可申请原因");
        expect(wrapper.get("form button").attributes("disabled")).toBeDefined();
      }
      wrapper.unmount();
    },
  );
  it("keeps one request through draft changes and double clicks, then requires explicit new intent", async () => {
    const { c, port, snapshot } = fixture();
    await c.refresh();
    const item = snapshot.catalog.find((i) => i.itemId === "diagnostics")!;
    c.select(item);
    const id = c.state.requestId;
    c.change("host", { kind: "text", value: "example.invalid" });
    await c.prepare();
    c.change("count", { kind: "integer", value: "3.0" });
    expect(c.state.plan).toBeNull();
    await c.prepare();
    expect(c.state.requestId).toBe(id);
    await Promise.all([c.submit(), c.submit()]);
    expect(port.submit).toHaveBeenCalledTimes(1);
    expect(c.state.accepted).toBe(true);
    c.select(item);
    expect(c.state.requestId).toBe(id);
    c.select(item, true);
    expect(c.state.requestId).not.toBe(id);
  });
  it("rejects a preview that arrives after the selected catalog expires", async () => {
    const { c, port, snapshot } = fixture();
    await c.refresh();
    c.select(
      c.state.snapshot!.catalog.find((i) => i.itemId === "diagnostics")!,
    );
    let finish!: (plan: Plan) => void;
    port.preview.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    const pending = c.prepare();
    const item = snapshot.catalog.find((i) => i.itemId === "diagnostics")!;
    item.availability = "expired";
    item.display.requestability = "unknown";
    await c.refresh();
    finish(structuredClone(preview.requests[0].plan));
    await pending;
    expect(c.state.plan).toBeNull();
    expect(c.state.busy).toBe(false);
    await c.prepare();
    expect(port.preview).toHaveBeenCalledTimes(1);
  });
  it("keeps the exact ambiguous submission retry after its catalog item disappears", async () => {
    const { c, port, snapshot } = fixture();
    const wrapper = mount(SelfService, { props: { controller: c } });
    await flushPromises();
    c.select(
      c.state.snapshot!.catalog.find((i) => i.itemId === "diagnostics")!,
    );
    await c.prepare();
    port.submit.mockRejectedValueOnce(new Error("response lost"));
    await c.submit();
    snapshot.catalog = [];
    await c.refresh();
    await flushPromises();
    expect(c.state.item).toBeNull();
    expect(c.state.page).toBe("tasks");
    const retry = wrapper
      .findAll("button")
      .find((button) => button.text() === "按原请求重试提交")!;
    await retry.trigger("click");
    await flushPromises();
    expect(port.submit.mock.calls[1]).toEqual(port.submit.mock.calls[0]);
    expect(c.state.accepted).toBe(true);
    wrapper.unmount();
  });
  it("retains the exact submission on lost response and clears secret inputs", async () => {
    const { c, port, snapshot } = fixture();
    await c.refresh();
    c.select(snapshot.catalog.find((i) => i.itemId === "diagnostics")!);
    c.change("credential", {
      kind: "secretReference",
      id: "canary",
      revision: "r1",
    });
    await c.prepare();
    port.submit.mockRejectedValueOnce(new Error("lost"));
    await c.submit();
    const first = port.submit.mock.calls[0][0];
    expect(c.state.uncertain).toBe(true);
    expect(c.state.fields.has("credential")).toBe(false);
    c.select(snapshot.catalog[0], true);
    expect(c.state.requestId).toBe(first.requestId);
    await c.submit();
    expect(port.submit.mock.calls[1][0]).toEqual(first);
    expect(c.state.accepted).toBe(true);
  });
  it.each([false, true])(
    "revises the redacted draft after authoritative rejection (previously lost: %s)",
    async (lost) => {
      const { c, port, snapshot } = fixture();
      await c.refresh();
      c.select(snapshot.catalog.find((i) => i.itemId === "diagnostics")!);
      c.change("credential", {
        kind: "secretReference",
        id: "canary",
        revision: "r1",
      });
      await c.prepare();
      const first = port.preview.mock.calls[0][0];
      if (lost) {
        port.submit.mockRejectedValueOnce(new Error("response lost"));
        await c.submit();
      }
      port.submit.mockRejectedValueOnce({
        code: "expired",
        message: "计划已过期，请重新预览",
      });
      await c.submit();
      expect(c.state.fields.has("credential")).toBe(false);
      expect(c.state.plan).toBeNull();
      await c.prepare();
      if (lost)
        expect(port.submit.mock.calls[1]).toEqual(port.submit.mock.calls[0]);
      const next = port.preview.mock.calls[1][0];
      expect(next.requestId).toBe(first.requestId);
      expect(next.revision).toBeGreaterThan(first.revision);
      expect(next.fields).not.toHaveProperty("credential");
    },
  );
  it("reconciles committed submissions and answers after lost responses", async () => {
    const { c, port, snapshot } = fixture();
    await c.refresh();
    c.select(snapshot.catalog.find((i) => i.itemId === "diagnostics")!);
    await c.prepare();
    port.submit.mockImplementationOnce(async (input) => {
      const task = structuredClone(preview.requests[0]);
      task.plan = {
        ...task.plan,
        requestId: input.requestId,
        planId: input.planId,
        digest: input.digest,
      };
      snapshot.requests.push(task);
      throw new Error("committed, response lost");
    });
    await c.submit();
    expect(c.state.uncertain).toBe(true);
    await c.refresh();
    expect(c.state.accepted).toBe(true);
    expect(c.state.uncertain).toBe(false);
    expect(c.state.error).toBe("");
    const task = snapshot.requests[0];
    port.respond.mockImplementationOnce(async () => {
      task.interactions[0].status = "answered";
      throw new Error("committed, response lost");
    });
    await c.respond(task, task.interactions[0].id, {
      kind: "confirmation",
      accepted: true,
    });
    expect(c.state.replyUnknown).toBe(true);
    await c.refresh();
    expect(c.state.replyUnknown).toBe(false);
    expect(c.state.error).toBe("");
    port.snapshot.mockRejectedValueOnce(new Error("unavailable"));
    await c.refresh();
    expect(c.state.error).toContain("无法读取");
    await c.refresh();
    expect(c.state.error).toBe("");
  });
  it("uses service validation errors and never falls back to success", async () => {
    const { c, port, snapshot } = fixture();
    await c.refresh();
    c.select(snapshot.catalog.find((i) => i.itemId === "diagnostics")!);
    port.preview.mockRejectedValueOnce({
      code: "catalog",
      message: "目录校验失败：Range",
    });
    await c.prepare();
    expect(c.state.error).toContain("Range");
    expect(c.state.plan).toBeNull();
    expect(port.submit).not.toHaveBeenCalled();
    port.snapshot.mockRejectedValueOnce(new Error("unavailable"));
    await c.refresh();
    expect(c.state.error).toContain("无法读取");
    expect(c.interactive).toBe(true);
  });
  it("retries an ambiguous answer with the same command identity", async () => {
    const { c, port, task } = fixture();
    await c.refresh();
    port.respond.mockRejectedValueOnce(new Error("lost"));
    await c.respond(task, task.interactions[0].id, {
      kind: "confirmation",
      accepted: true,
    });
    expect(c.state.replyUnknown).toBe(true);
    await c.respond(task, task.interactions[0].id, {
      kind: "confirmation",
      accepted: false,
    });
    expect(port.respond).toHaveBeenCalledTimes(1);
    await c.retryReply();
    expect(port.respond.mock.calls[1]).toEqual(port.respond.mock.calls[0]);
  });
  it("discards old preview and submit responses after a service restart", async () => {
    for (const operation of ["preview", "submit"] as const) {
      const { c, port, snapshot } = fixture();
      await c.refresh();
      c.select(snapshot.catalog.find((i) => i.itemId === "diagnostics")!);
      if (operation === "submit") await c.prepare();
      let finish = () => {};
      if (operation === "preview")
        port.preview.mockImplementationOnce(
          () =>
            new Promise<Plan>((resolve) => {
              finish = () => resolve(structuredClone(preview.requests[0].plan));
            }),
        );
      else
        port.submit.mockImplementationOnce(
          () =>
            new Promise<RequestView>((resolve) => {
              finish = () => resolve(structuredClone(preview.requests[0]));
            }),
        );
      const pending = operation === "preview" ? c.prepare() : c.submit();
      snapshot.instanceId = "restarted";
      await c.refresh();
      finish();
      await pending;
      expect(c.state.plan).toBeNull();
      expect(c.state.accepted).toBe(false);
      expect(c.state.busy).toBe(false);
      expect(c.state.snapshot?.requests).toEqual([]);
    }
  });
  it("unmounting the view does not send an interaction cancellation", async () => {
    const { c, port, snapshot } = fixture();
    const wrapper = mount(SelfService, { props: { controller: c } });
    await flushPromises();
    c.select(snapshot.catalog.find((i) => i.itemId === "diagnostics")!);
    await c.prepare();
    await c.submit();
    wrapper.unmount();
    expect(port.respond).not.toHaveBeenCalled();
    expect(c.state.snapshot?.requests).toHaveLength(1);
  });
});
it("form sends original numeric tokens and explicit false, without applying defaults", async () => {
  const item = preview.catalog.find((i) => i.itemId === "diagnostics")!;
  const values = new Map();
  const wrapper = mount(ParameterForm, {
    props: { fields: item.fields, values, disabled: false, prefix: "test" },
  });
  expect(values.size).toBe(0);
  await wrapper.get("#test-count").setValue("3.0000000000000000001");
  expect(wrapper.emitted("change")?.[0]).toEqual([
    "count",
    { kind: "integer", value: "3.0000000000000000001" },
  ]);
  await wrapper.get("#test-detail").setValue("false");
  expect(wrapper.emitted("change")?.[1]).toEqual([
    "detail",
    { kind: "boolean", value: false },
  ]);
  expect(wrapper.get("#test-credential").attributes("type")).toBe("password");
});
it("browser uses the same readonly pages and task views without a fake service", async () => {
  const c = createController(
    null,
    () => {
      throw new Error("no identity in preview");
    },
    structuredClone(preview),
  );
  const wrapper = mount(SelfService, { props: { controller: c } });
  c.select(preview.catalog.find((i) => i.itemId === "diagnostics")!);
  await flushPromises();
  expect(wrapper.get("#draft-host").attributes("disabled")).toBeDefined();
  c.navigate("tasks");
  c.state.taskId = preview.requests.find(
    (r) => r.status === "approval",
  )!.plan.requestId;
  await flushPromises();
  expect(wrapper.text()).toContain("此处没有管理员批准入口");
  expect(
    wrapper
      .findAll(".interaction-card button")
      .every((button) => button.attributes("disabled") !== undefined),
  ).toBe(true);
});
it("a delayed snapshot cannot erase a newer acceptance receipt", async () => {
  const { c, port, snapshot } = fixture();
  await c.refresh();
  c.select(snapshot.catalog.find((i) => i.itemId === "diagnostics")!);
  await c.prepare();
  let finish = () => {};
  port.snapshot.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        finish = () => resolve(structuredClone(snapshot));
      }),
  );
  const refresh = c.refresh();
  await c.submit();
  finish();
  await refresh;
  expect(c.state.snapshot?.requests).toHaveLength(1);
  expect(c.state.accepted).toBe(true);
});
