import { mount, flushPromises } from "@vue/test-utils";
import { expect, it, vi } from "vitest";
import { computed, shallowRef } from "vue";
import ExecutionActivity from "./operation-ui/OperationActivity.vue";
import { createAssistant } from "./controller";
import { taskPresentation } from "./execution-presentation";
import { copyText } from "./clipboard";
import { offer } from "../../../../tests/self-service/support";
import fixtures from "../../../../tests/assistant/execution-fixtures.json";
import type {
  BackendTaskView,
  ExecutionTaskDetails,
} from "@rss-mdm-agent/execution-bindings/task-details";

function assistant() {
  const c = createAssistant(undefined, () => "id");
  return {
    ...c,
    view: computed(
      () =>
        ({
          generation: "generation-1",
          namespace: {
            authorityId: "a",
            tenantId: "t",
            principalId: "p",
            sessionId: "session",
          },
        }) as NonNullable<typeof c.view.value>,
    ),
  };
}

function task(
  phase: ExecutionTaskDetails["status"]["phase"],
  assessment: ExecutionTaskDetails["status"]["assessment"] = null,
) {
  const value = structuredClone(
    fixtures.outcomeUnknown,
  ) as ExecutionTaskDetails;
  value.action.initiator = {
    kind: "backend",
    trigger: structuredClone(fixtures.proposed.trigger),
  } as ExecutionTaskDetails["action"]["initiator"];
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

it("refreshes a card and its open drawer from one authorized snapshot and invalidates actions on read failure", async () => {
  const c = assistant();
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
    await vi.waitFor(
      () => expect(wrapper.find(".operation-card").exists()).toBe(true),
      { timeout: 5000 },
    );
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
      .find((b) => b.text() === "查询原任务")!
      .trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain("已核实符合预期");
    expect(wrapper.get(".execution-phase").text()).toBe("执行结果已核实");
    read.mockRejectedValueOnce(new Error("offline"));
    await wrapper
      .findAll("button")
      .find((b) => b.text() === "查询原任务")!
      .trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain("结果未知");
    expect(wrapper.text()).toContain("当前内容不能作为执行依据");
    expect(read.mock.calls.every(([id]) => id === "tool")).toBe(true);
    await wrapper.setProps({ visible: false });
    expect(wrapper.find("dialog").exists()).toBe(false);
  } finally {
    wrapper.unmount();
    c.dispose();
  }
});

it("discards a late task read after the view has been hidden", async () => {
  const c = assistant();
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
  const c = assistant();
  c.state.selected = "session";
  const snapshot = shallowRef({
    generation: "generation-1",
    namespace: c.view.value!.namespace,
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
  const c = assistant();
  const request = offer();
  const read = vi.spyOn(c, "executionDetails").mockResolvedValue({
    kind: "pending",
    value: {
      recordedAtUnixMs: 1000,
      cancellationControl: null,
      retryOf: null,
      action: { kind: "offered", offer: request },
      revision: 1,
      state: "awaitingConfirmation",
      risk: null,
      confirmation: null,
      failure: null,
      trigger: structuredClone(fixtures.proposed.trigger) as Extract<
        BackendTaskView,
        { kind: "pending" }
      >["value"]["trigger"],
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
    expect(wrapper.text()).toContain("install");
    expect(wrapper.text()).toContain("固定软件 1.0");
    await wrapper
      .findAll("button")
      .find((b) => b.text() === "确认操作")!
      .trigger("click");
    await flushPromises();
    expect(confirm).toHaveBeenCalledWith(request);
    read.mockResolvedValueOnce({
      kind: "pending",
      value: {
        recordedAtUnixMs: 1000,
        cancellationControl: null,
        retryOf: null,
        action: { kind: "offered", offer: request },
        revision: 2,
        state: "failed",
        risk: null,
        confirmation: null,
        failure: "revoked",
        trigger: structuredClone(fixtures.proposed.trigger) as Extract<
          BackendTaskView,
          { kind: "pending" }
        >["value"]["trigger"],
      },
    });
    await wrapper
      .findAll("button")
      .find((b) => b.text() === "查询原任务")!
      .trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain("revoked");
    expect(
      wrapper
        .findAll("button")
        .find((b) => b.text() === "确认操作")!
        .attributes("disabled"),
    ).toBeDefined();
    expect(wrapper.text()).not.toContain("撤销请求");
  } finally {
    wrapper.unmount();
    c.dispose();
  }
});

it("opens execution details only through the parent selection without requerying or owning a second toggle", async () => {
  const c = assistant();
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
