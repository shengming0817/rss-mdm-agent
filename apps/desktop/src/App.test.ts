import { mount } from "@vue/test-utils";
import { expect, it, vi } from "vitest";
import App from "./App.vue";
it("runs the whole sample without network or a native bridge", async () => {
  const fetch = vi
    .spyOn(globalThis, "fetch")
    .mockRejectedValue(new Error("unexpected network"));
  const wrapper = mount(App);
  expect(wrapper.text()).toContain("展示样本，无真实执行");
  await wrapper.findAll("nav button")[1].trigger("click");
  await wrapper.get("textarea").setValue("hello");
  await wrapper.get("form").trigger("submit");
  expect(wrapper.get("textarea").element.value).toBe("");
  expect(wrapper.text()).toContain("hello");
  await wrapper.get(".state-toggle").trigger("click");
  expect(wrapper.get("textarea").attributes("disabled")).toBeDefined();
  await wrapper.get('[data-action="cancel"]').trigger("click");
  expect(wrapper.text()).toContain("已取消样本等待");
  expect(fetch).not.toHaveBeenCalled();
});
