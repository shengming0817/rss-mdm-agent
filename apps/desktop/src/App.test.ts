import { mount } from "@vue/test-utils";
import { expect, it, vi } from "vitest";
import App from "./App.vue";
it("replaces the old sample with an explicitly read-only browser preview", async () => {
  const fetch = vi
    .spyOn(globalThis, "fetch")
    .mockRejectedValue(new Error("unexpected network"));
  const wrapper = mount(App);
  expect(wrapper.text()).toContain("浏览器只读预览");
  expect(wrapper.text()).toContain("软件中心");
  expect(wrapper.text()).toContain("工具中心");
  const assistant = wrapper
    .findAll("button")
    .find((button) => button.text() === "AI 助手")!;
  expect(assistant).toBeDefined();
  await assistant.trigger("click");
  expect(wrapper.text()).toContain("AI 服务未连接");
  expect(wrapper.text()).not.toContain("消息与输入");
  expect(fetch).not.toHaveBeenCalled();
});
