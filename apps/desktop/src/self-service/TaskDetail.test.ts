import { mount } from "@vue/test-utils";
import { expect, test } from "vitest";
import TaskDetail from "./TaskDetail.vue";
import { executionTask } from "../../../../tests/self-service/support";
test("exit and independent effect stay separate and cancellation is a request", async () => {
  const task = executionTask();
  task.status.assessment = null;
  const wrapper = mount(TaskDetail, { props: { task, disabled: false } });
  expect(wrapper.text()).toContain("结果未知时不要重复派发");
  expect(wrapper.text()).not.toContain("安装成功");
  await wrapper.get("button").trigger("click");
  expect(wrapper.emitted("cancel")).toHaveLength(1);
});
test("backend authority and actual AI origin are presented without client approval", () => {
  const task = executionTask();
  task.action.initiator = {
    kind: "backend",
    task: "task-7",
    attempt: "attempt-7",
    trigger: {
      kind: "ai",
      osSession: {
        device: "device",
        account: { platform: "macos", subject: "501" },
        session: "native-session",
      },
      config: { id: "config", revision: "r1" },
      conversation: "conversation-7",
      toolCall: "call-7",
    },
  };
  const wrapper = mount(TaskDetail, { props: { task, disabled: false } });
  for (const fact of [
    "task-7",
    "attempt-7",
    "native-session",
    "conversation-7",
    "call-7",
  ])
    expect(wrapper.text()).toContain(fact);
  expect(wrapper.findAll("button").map((b) => b.text())).toEqual([
    "请求取消原任务",
  ]);
});

test("human details expose unavailable detection and mechanism evidence", () => {
  const task = executionTask();
  task.status.software = "detectionUnavailable";
  const wrapper = mount(TaskDetail, { props: { task, disabled: false } });
  expect(wrapper.text()).toContain("检测");
  expect(wrapper.find(".software-diagnostic").exists()).toBe(true);
  expect(wrapper.text()).toContain("不要重复派发");
});
