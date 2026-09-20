import { mount } from "@vue/test-utils";
import { expect, test } from "vitest";
import TaskDetail from "./TaskDetail.vue";
import preview from "./preview";
import type { RequestView } from "./types";
test("an expired prompt stops offering confirmation while business cancellation remains separate", async () => {
  const task: RequestView = {
    ...preview.requests[0]!,
    interactions: [
      {
        id: "expiry-test",
        status: "pending",
        expiresAtUnixMs: 2000,
        message: "Test notice",
        options: [],
        kind: { kind: "userConfirmation", purpose: "continue" },
      },
    ],
  };
  const wrapper = mount(TaskDetail, {
    props: { task, item: undefined, disabled: false, now: 1000 },
  });
  expect(wrapper.findAll("button").some((b) => b.text() === "确认计划")).toBe(
    true,
  );
  await wrapper.setProps({ now: 2000 });
  expect(wrapper.text()).toContain("按本机时间已过期");
  expect(wrapper.findAll("button").some((b) => b.text() === "确认计划")).toBe(
    false,
  );
});

test("approval shows the frozen actor, authority and AI account separately from run-as", async () => {
  const task = structuredClone(preview.requests[0]!);
  task.status = "approval";
  task.plan.actor = "request-actor";
  task.plan.authority = {
    kind: "enterprise",
    id: "authority-a",
    tenant: "tenant-a",
  };
  task.plan.initiator = {
    kind: "ai",
    provider: "codex",
    osSession: {
      device: "origin-device",
      account: { platform: "macos", subject: "os-user" },
      session: "os-session",
    },
    providerAccount: {
      account: "ai-account",
      config: { id: "ai-config", revision: "r7" },
    },
    conversation: "conversation-7",
    toolCall: "tool-call-7",
  };
  task.plan.runAs = "execution-user";
  const wrapper = mount(TaskDetail, {
    props: { task, item: undefined, disabled: false, now: 1000 },
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
    "ai-account",
    "ai-config",
    "r7",
    "conversation-7",
    "tool-call-7",
  ])
    expect(origin.text()).toContain(value);
  expect(wrapper.text()).toContain("execution-user");
  await wrapper
    .findAll("button")
    .find((b) => b.text() === "批准此测试计划一次")!
    .trigger("click");
  expect(wrapper.emitted("approve")).toHaveLength(1);
  await wrapper.setProps({
    task: {
      ...task,
      plan: {
        ...task.plan,
        initiator: { kind: "human", osSession: task.plan.initiator.osSession },
      },
    },
  });
  expect(origin.text()).toContain("人工发起");
  expect(origin.text()).not.toContain("ai-account");
});
