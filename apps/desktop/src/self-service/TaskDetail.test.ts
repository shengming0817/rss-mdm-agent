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
