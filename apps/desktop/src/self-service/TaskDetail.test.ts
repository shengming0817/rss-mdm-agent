import { mount } from "@vue/test-utils";
import { expect, test } from "vitest";
import TaskDetail from "./TaskDetail.vue";
import { executionTask } from "./testing";
import type { RequestView } from "./types";
test("an expired prompt stops offering confirmation while business cancellation remains separate", async () => {
  const task: RequestView = {
    ...executionTask(),
    confirmation: {
      status: "pending",
      expiresAtUnixMs: 2000,
      message: "Test notice",
    },
  };
  const wrapper = mount(TaskDetail, {
    props: { task, disabled: false, now: 1000 },
  });
  expect(wrapper.findAll("button").some((b) => b.text() === "确认并执行")).toBe(
    true,
  );
  await wrapper.setProps({ now: 2000 });
  expect(wrapper.text()).toContain("按本机时间已过期");
  expect(wrapper.findAll("button").some((b) => b.text() === "确认并执行")).toBe(
    false,
  );
});

test("approval shows the frozen actor, authority and AI account separately from run-as", async () => {
  const task = structuredClone(executionTask());
  task.status = "confirmation";
  task.action.actor = "request-actor";
  task.action.authority = {
    kind: "enterprise",
    id: "authority-a",
    tenant: "tenant-a",
  };
  task.action.initiator = {
    kind: "ai",
    provider: "codex",
    osSession: {
      device: "origin-device",
      account: { platform: "macos", subject: "os-user" },
      session: "os-session",
    },
    config: { id: "ai-config", revision: "r7" },
    conversation: "conversation-7",
    toolCall: "tool-call-7",
  };
  task.action.runAs = "execution-user";
  const wrapper = mount(TaskDetail, {
    props: { task, disabled: false, now: 1000 },
  });
  const origin = wrapper.get('[aria-label="冻结的请求来源"]');
  for (const value of [
    "request-actor",
    "authority-a",
    "tenant-a",
    "AI 发起",
    "codex",
    "origin-device",
    "os-user",
    "os-session",
    "ai-config",
    "r7",
    "conversation-7",
    "tool-call-7",
  ])
    expect(origin.text()).toContain(value);
  expect(wrapper.text()).toContain("execution-user");
  await wrapper
    .findAll("button")
    .find((b) => b.text() === "确认并执行")!
    .trigger("click");
  expect(wrapper.emitted("confirm")).toHaveLength(1);
  await wrapper.setProps({
    task: {
      ...task,
      action: {
        ...task.action,
        initiator: {
          kind: "human",
          osSession: task.action.initiator.osSession,
        },
      },
    },
  });
  expect(origin.text()).toContain("人工发起");
  expect(origin.text()).not.toContain("ai-account");
});

test("exact action summary precedes confirmation and only business cancellation is offered", () => {
  const wrapper = mount(TaskDetail, {
    props: {
      task: executionTask(),
      disabled: false,
      now: 1000,
    },
  });
  expect(wrapper.html().indexOf("action-summary")).toBeLessThan(
    wrapper.html().indexOf("确认并执行"),
  );
  expect(wrapper.text()).not.toContain("取消此交互");
});

test("policy authorization never exposes a self-confirmation control", () => {
  const task = executionTask();
  task.status = "approval" as RequestView["status"];
  const wrapper = mount(TaskDetail, {
    props: { task, disabled: false, now: 1000 },
  });
  expect(wrapper.text()).toContain("等待策略授权");
  expect(wrapper.findAll("button").some((b) => b.text() === "确认并执行")).toBe(
    false,
  );
});

test.each([
  [0, "纯计算"],
  [1, "有界非敏感只读"],
  [2, "需要用户确认"],
  [3, "AI 默认阻止"],
  [null, "无可信分类"],
] as const)(
  "risk %s describes its behavior without granting permission",
  (level, label) => {
    const task = executionTask();
    task.action.riskLevel = level;
    const wrapper = mount(TaskDetail, {
      props: { task, disabled: false, now: 1000 },
    });
    expect(wrapper.text()).toContain(label);
    expect(wrapper.text()).toContain("风险等级不授予权限");
  },
);
