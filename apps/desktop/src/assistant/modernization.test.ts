import { mount, flushPromises } from "@vue/test-utils";
import { expect, it, vi } from "vitest";
import { computed, shallowRef } from "vue";
import ExecutionActivity from "./ExecutionActivity.vue";
import { createAssistant } from "./controller";
import { taskPresentation } from "./execution-presentation";
import { copyText } from "./clipboard";
import { offer } from "../../../../tests/self-service/support";
import fixtures from "../../../../tests/assistant/execution-fixtures.json";
import type {
  BackendTaskView,
  ExecutionTaskDetails,
} from "@rss-mdm-agent/execution-bindings/task-details";

function task(
  phase: ExecutionTaskDetails["status"]["phase"],
  assessment: ExecutionTaskDetails["status"]["assessment"] = null,
) {
  const value = structuredClone(
    fixtures.outcomeUnknown,
  ) as ExecutionTaskDetails;
  value.status.phase = phase;
  value.status.assessment = assessment;
  return value;
}

it.each([
  ["satisfied", "已核实符合预期"],
  ["notSatisfied", "已核实未达预期"],
  ["noEffect", "已核实无效果"],
  ["unknown", "效果仍需核对"],
] as const)("distinguishes verified assessment %s", (assessment, label) => {
  expect(taskPresentation(task("verified", assessment)).label).toBe(label);
});

it("does not infer success from cancellation requests, process exit or software observation", () => {
  const value = task("executionEnded", "unknown");
  value.status.cancelRequested = true;
  value.status.stopOutcome = "acknowledged";
  value.status.software = "desiredStateObserved";
  expect(taskPresentation(value).label).toContain("等待效果验证");
  expect(taskPresentation(value).stage).toContain("不代表后台活动已终止");
  expect(taskPresentation(value).cancel).toContain("已请求取消");
});

it("refreshes a card and its open drawer from one authorized snapshot and retains it on read failure", async () => {
  const c = createAssistant(undefined, () => "id");
  c.state.selected = "session";
  const read = vi
    .spyOn(c, "executionDetails")
    .mockResolvedValue({ kind: "execution", value: task("running") });
  const wrapper = mount(ExecutionActivity, {
    props: {
      controller: c,
      operationId: "tool",
      detailsOpen: false,
      recorded: true,
      visible: true,
      now: 1000,
    },
  });
  try {
    await flushPromises();
    await wrapper
      .findAll("button")
      .find((b) => b.text() === "查看设备操作")!
      .trigger("click");
    await wrapper.setProps({ detailsOpen: true });
    read.mockResolvedValueOnce({
      kind: "execution",
      value: task("verified", "satisfied"),
    });
    await wrapper
      .findAll("button")
      .find((b) => b.text() === "刷新执行状态")!
      .trigger("click");
    await flushPromises();
    expect(wrapper.get(".task-status").text()).toBe("已核实符合预期");
    expect(wrapper.get(".execution-phase").text()).toBe("执行结果已核实");
    read.mockRejectedValueOnce(new Error("offline"));
    await wrapper
      .findAll("button")
      .find((b) => b.text() === "刷新执行状态")!
      .trigger("click");
    await flushPromises();
    expect(wrapper.get(".task-status").text()).toBe("已核实符合预期");
    expect(wrapper.text()).toContain("当前显示上次已读取记录");
    expect(read.mock.calls.every(([id]) => id === "tool")).toBe(true);
    await wrapper.setProps({ visible: false });
    expect(wrapper.find("dialog").exists()).toBe(false);
  } finally {
    wrapper.unmount();
    c.dispose();
  }
});

it("discards a late task read after the view has been hidden", async () => {
  const c = createAssistant(undefined, () => "id");
  let finish!: (value: BackendTaskView) => void;
  vi.spyOn(c, "executionDetails").mockImplementation(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  const wrapper = mount(ExecutionActivity, {
    props: {
      controller: c,
      operationId: "tool",
      detailsOpen: false,
      recorded: true,
      visible: true,
      now: 1000,
    },
  });
  try {
    await wrapper.setProps({ visible: false });
    finish({ kind: "execution", value: task("verified", "satisfied") });
    await flushPromises();
    expect(wrapper.find(".execution-activity").exists()).toBe(false);
  } finally {
    wrapper.unmount();
    c.dispose();
  }
});

it("keeps one pending task read while conversation snapshots stream in the same generation", async () => {
  const c = createAssistant(undefined, () => "id");
  c.state.selected = "session";
  const snapshot = shallowRef({
    generation: "generation-1",
  } as NonNullable<typeof c.view.value>);
  const controller = { ...c, view: computed(() => snapshot.value) };
  let finish!: (value: BackendTaskView) => void;
  const read = vi.spyOn(controller, "executionDetails").mockImplementation(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  const wrapper = mount(ExecutionActivity, {
    props: {
      controller,
      operationId: "tool",
      detailsOpen: false,
      recorded: true,
      visible: true,
      now: 1000,
    },
  });
  try {
    for (let token = 0; token < 20; token++) {
      snapshot.value = { ...snapshot.value };
      await flushPromises();
    }
    expect(read).toHaveBeenCalledTimes(1);
    expect(read.mock.calls[0][1].aborted).toBe(false);
    finish({ kind: "execution", value: task("running") });
    await flushPromises();
    expect(wrapper.find(".execution-activity").exists()).toBe(true);
  } finally {
    wrapper.unmount();
    c.dispose();
  }
});

it("reports clipboard rejection without a native fallback or clipboard read", async () => {
  const write = vi
    .spyOn(navigator.clipboard, "writeText")
    .mockRejectedValueOnce(new Error("denied"));
  expect(await copyText("code")).toBe(false);
  write.mockResolvedValueOnce(undefined);
  expect(await copyText("code")).toBe(true);
  expect(write.mock.calls).toEqual([["code"], ["code"]]);
});

it("keeps preparation actions and revocation visible in the modern task card", async () => {
  const c = createAssistant(undefined, () => "id");
  const request = offer();
  const read = vi.spyOn(c, "executionDetails").mockResolvedValue({
    kind: "pending",
    value: {
      offer: request,
      revision: 1,
      state: "proposed",
      failure: null,
      trigger: { kind: "automatic" },
    },
  });
  const confirm = vi
    .spyOn(c, "confirmPreparation")
    .mockResolvedValue(undefined);
  const wrapper = mount(ExecutionActivity, {
    props: {
      controller: c,
      operationId: "tool",
      detailsOpen: false,
      recorded: true,
      visible: true,
      now: 1000,
    },
  });
  try {
    await flushPromises();
    expect(wrapper.text()).toContain("安装");
    expect(wrapper.text()).toContain("固定软件 1.0");
    await wrapper
      .findAll("button")
      .find((b) => b.text() === "确认上述操作")!
      .trigger("click");
    expect(confirm).toHaveBeenCalledWith(request);
    read.mockResolvedValueOnce({
      kind: "pending",
      value: {
        offer: request,
        revision: 2,
        state: "failed",
        failure: "revoked",
        trigger: { kind: "automatic" },
      },
    });
    await wrapper
      .findAll("button")
      .find((b) => b.text() === "刷新状态")!
      .trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain("后台授权已撤销");
    expect(wrapper.text()).not.toContain("确认上述操作");
    expect(wrapper.text()).not.toContain("撤销请求");
  } finally {
    wrapper.unmount();
    c.dispose();
  }
});

it("opens execution details only through the parent selection without requerying or owning a second toggle", async () => {
  const c = createAssistant(undefined, () => "id");
  c.state.selected = "session";
  const read = vi
    .spyOn(c, "executionDetails")
    .mockResolvedValue({ kind: "execution", value: task("running") });
  const host = document.createElement("div");
  document.body.append(host);
  const w = mount(ExecutionActivity, {
    attachTo: document.body,
    props: {
      controller: c,
      operationId: "tool",
      recorded: true,
      visible: true,
      now: 1000,
      detailsOpen: false,
      detailsHost: host,
    },
  });
  try {
    await flushPromises();
    await w
      .findAll("button")
      .find((b) => b.text() === "查看设备操作")!
      .trigger("click");
    expect(w.emitted("open")).toEqual([["tool"]]);
    expect(host.querySelector(".execution-details")).toBeNull();
    await w.setProps({ detailsOpen: true });
    expect(host.querySelector(".execution-details")).not.toBeNull();
    expect(read).toHaveBeenCalledTimes(1);
    await w.setProps({ detailsOpen: false });
    expect(host.querySelector(".execution-details")).toBeNull();
  } finally {
    w.unmount();
    host.remove();
    c.dispose();
  }
});
